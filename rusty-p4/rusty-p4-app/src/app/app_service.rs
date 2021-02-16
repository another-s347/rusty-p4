use crate::app::P4app;
use async_trait::async_trait;
use crate::core::DefaultContext;
use crate::event::{PacketReceived, NorthboundRequest, Event};
use failure::_core::marker::PhantomData;
use crate::app::common::CommonState;
use futures::{StreamExt, Future, SinkExt};
use crate::service::{Service};
use std::collections::HashMap;
use std::any::{TypeId, Any};
use std::rc::Rc;
use std::fmt::{Debug, Formatter, Error};
use crate::core::context::Context;

pub struct AppServiceBuilder<E, C = DefaultContext<E>> {
    last_receiver:tokio::sync::mpsc::Receiver<_M<E, C>>,
    context_senders:Vec<tokio::sync::oneshot::Sender<C>>,
    m_senders:Vec<tokio::sync::mpsc::Sender<_M<E, C>>>,
    first_sender:tokio::sync::mpsc::Sender<_M<E, C>>,
    services:HashMap<TypeId,Rc<dyn Any>>
}

impl<E, C> AppServiceBuilder<E, C> where E:Event, C: Context<E> {
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

    pub fn with<T>(&mut self,app:T) where T:P4app<E, C> {
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
        where T: P4app<E, C> + Service
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

    pub fn build(self) -> AppService<E, C> {
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

pub struct AppService<E, C> {
    context_senders:Vec<tokio::sync::oneshot::Sender<C>>,
    m_senders:Vec<tokio::sync::mpsc::Sender<_M<E, C>>>,
    first_sender:tokio::sync::mpsc::Sender<_M<E, C>>
}

pub struct AppContainer<T,E,C> {
    pub(crate) context_receiver:tokio::sync::oneshot::Receiver<C>,
    pub(crate) m_receiver:tokio::sync::mpsc::Receiver<_M<E, C>>,
    pub(crate) m_sender:tokio::sync::mpsc::Sender<_M<E, C>>,
    pub(crate) app:T,
}

impl<T,E,C> AppContainer<T,E,C> where T:P4app<E,C>,E:Event,C: Context<E> {
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
impl<E, C> P4app<E, C> for AppService<E, C> where E:Event,C: Context<E>+Clone {
    async fn on_start(self: &mut Self, ctx: &mut C) {
        for x in self.context_senders.drain(0..) {
            x.send(ctx.clone());
        }
    }

    async fn on_packet(
        self: &mut Self,
        packet: PacketReceived,
        ctx: &mut C,
    ) -> Option<PacketReceived> {
        self.first_sender.send(_M::Packet(packet)).await;
        None
    }

    async fn on_event(self: &mut Self, event: E, ctx: &mut C) -> Option<E> {
        self.first_sender.send(_M::Event(event)).await;
        None
    }

    async fn on_context_update(self: &mut Self, ctx: &mut C) {
        for s in self.m_senders.iter_mut() {
            s.send(_M::Context(ctx.clone())).await.unwrap();
        }
    }
}

pub(crate) enum _M<E, C> {
    Event(E),
    Packet(PacketReceived),
    Context(C)
}

impl<E, C> Debug for _M<E, C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        Ok(())
    }
}