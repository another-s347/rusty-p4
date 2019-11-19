use crate::app::P4app;
use async_trait::async_trait;
use crate::core::Context;
use crate::event::{PacketReceived, NorthboundRequest, Event};
use failure::_core::marker::PhantomData;
use crate::app::common::CommonState;
use futures::{StreamExt, Future, SinkExt};
use crate::service::{Service};
use std::collections::HashMap;
use std::any::{TypeId, Any};
use std::rc::Rc;
use std::fmt::{Debug, Formatter, Error};

pub struct AppServiceBuilder<E> {
    last_receiver:tokio::sync::mpsc::Receiver<_M<E>>,
    context_senders:Vec<tokio::sync::oneshot::Sender<Context<E>>>,
    m_senders:Vec<tokio::sync::mpsc::Sender<_M<E>>>,
    first_sender:tokio::sync::mpsc::Sender<_M<E>>,
    services:HashMap<TypeId,Rc<dyn Any>>
}

impl<E> AppServiceBuilder<E> where E:Event {
    pub fn new() -> Self {
        let (s,mut c) = tokio::sync::mpsc::channel(10240);
        AppServiceBuilder {
            last_receiver: c,
            context_senders: vec![],
            m_senders: vec![s.clone()],
            first_sender: s,
            services: Default::default()
        }
    }

    pub fn with<T>(&mut self,app:T) where T:P4app<E> {
        let (s,mut c) = tokio::sync::mpsc::channel(10240);
        let c = std::mem::replace(&mut self.last_receiver,c);
        self.m_senders.push(s.clone());
        let (context_sender,context_receiver) = tokio::sync::oneshot::channel();
        self.context_senders.push(context_sender);
        tokio::spawn(AppContainer {
            context_receiver,
            m_receiver: c,
            m_sender: s,
            app
        }.run());
    }

    pub fn with_service<T>(&mut self, mut app:T) -> T::ServiceType
        where T: P4app<E> + Service
    {
        let (s,mut c) = tokio::sync::mpsc::channel(10240);
        let c = std::mem::replace(&mut self.last_receiver,c);
        self.m_senders.push(s.clone());
        let (context_sender,context_receiver) = tokio::sync::oneshot::channel();
        self.context_senders.push(context_sender);
        let service = app.get_service();
        self.services.insert(service.type_id(),Rc::new(service.clone()));
        tokio::spawn(AppContainer {
            context_receiver,
            m_receiver: c,
            m_sender: s,
            app
        }.run());
        service
    }

    pub fn build(self) -> AppService<E> {
        let mut last_receiver = self.last_receiver;
        tokio::spawn(async move {
            while let Some(_) = last_receiver.next().await {}
        });
        AppService {
            context_senders: self.context_senders,
            m_senders: self.m_senders,
            first_sender: self.first_sender
        }
    }

    pub fn get_service<T:Clone+'static>(&self) -> Option<T> {
        self.services.get(&TypeId::of::<T>()).and_then(|x|{
            Rc::downcast(x.clone()).ok()
        }).map(|x:Rc<T>|{
            x.as_ref().clone()
        })
    }
}

pub struct AppService<E> {
    context_senders:Vec<tokio::sync::oneshot::Sender<Context<E>>>,
    m_senders:Vec<tokio::sync::mpsc::Sender<_M<E>>>,
    first_sender:tokio::sync::mpsc::Sender<_M<E>>
}

pub struct AppContainer<T,E> {
    pub(crate) context_receiver:tokio::sync::oneshot::Receiver<Context<E>>,
    pub(crate) m_receiver:tokio::sync::mpsc::Receiver<_M<E>>,
    pub(crate) m_sender:tokio::sync::mpsc::Sender<_M<E>>,
    pub(crate) app:T,
}

impl<T,E> AppContainer<T,E> where T:P4app<E>,E:Event {
    async fn run(mut self) {
        let mut context = self.context_receiver.await.unwrap();
        self.app.on_start(&mut context).await;
        while let Some(p) = self.m_receiver.next().await {
            match p {
                _M::Event(e)=>{
                    if let Some(e) = self.app.on_event(e,&mut context).await {
                        self.m_sender.send(_M::Event(e)).await;
                    }
                }
                _M::Packet(p)=>{
                    if let Some(p) = self.app.on_packet(p,&mut context).await {
                        self.m_sender.send(_M::Packet(p)).await;
                    }
                }
                _M::Context(new_ctx)=>{
                    context = new_ctx;
                    self.app.on_context_update(&mut context).await;
                }
            }
        }
    }
}

#[async_trait]
impl<E> P4app<E> for AppService<E> where E:Event {
    async fn on_start(self: &mut Self, ctx: &mut Context<E>) {
        for x in self.context_senders.drain(0..) {
            x.send(ctx.clone());
        }
    }

    async fn on_packet(
        self: &mut Self,
        packet: PacketReceived,
        ctx: &mut Context<E>,
    ) -> Option<PacketReceived> {
        self.first_sender.send(_M::Packet(packet)).await;
        None
    }

    async fn on_event(self: &mut Self, event: E, ctx: &mut Context<E>) -> Option<E> {
        self.first_sender.send(_M::Event(event)).await;
        None
    }

    async fn on_context_update(self: &mut Self, ctx: &mut Context<E>) {
        for s in self.m_senders.iter_mut() {
            s.send(_M::Context(ctx.clone())).await.unwrap();
        }
    }
}

pub(crate) enum _M<E> {
    Event(E),
    Packet(PacketReceived),
    Context(Context<E>)
}

impl<E> Debug for _M<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        Ok(())
    }
}