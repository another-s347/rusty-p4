use crate::app::P4app;
use crate::context::ContextHandle;
use crate::event::{Event, PacketReceived};
use failure::_core::marker::PhantomData;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::collections::LinkedList;

pub struct AsyncAppsBuilder<E> {
    apps: LinkedList<(u8, &'static str, Arc<dyn AsyncApp<E>>)>,
}

impl<E> AsyncAppsBuilder<E> where E: Event {
    pub fn new() -> AsyncAppsBuilder<E> {
        AsyncAppsBuilder {
            apps: LinkedList::new()
        }
    }

    pub fn with<T>(&mut self, priority: u8, name: &'static str, app: T) where T: AsyncApp<E> {
        self.insert(priority,name,Arc::new(app));
    }

    pub fn with_sync<T>(&mut self, priority: u8, name: &'static str, app: T) where T: P4app<E> + Send {
        self.insert(priority,name,Arc::new(Mutex::new(app)));
    }

    pub fn with_service<T>(&mut self, priority: u8, name: &'static str, app: T) -> Arc<T> where T: AsyncApp<E> {
        let obj = Arc::new(app);
        let t: Arc<T> = obj.clone();
        self.insert(priority,name,obj);
        t
    }

    pub fn with_sync_service<T>(&mut self, priority: u8, name: &'static str, app: T) -> Arc<Mutex<T>> where T: P4app<E> + Send {
        let a = Arc::new(Mutex::new(app));
        let b: Arc<Mutex<T>> = a.clone();
        self.insert(priority,name,a);
        b
    }

    fn insert<T>(&mut self, mut priority: u8, name: &'static str, app: Arc<T>)
        where
            T: AsyncApp<E>,
    {
        let mut iter = self.apps.iter_mut();
        while let Some((p, name, _)) = iter.next() {
            if priority > *p {
                iter.insert_next((priority, name, app));
                break;
            } else if priority == *p {
                priority -= 1;
                break;
            }
        }
    }

    pub fn build(mut self) -> AsyncAppsBase<E> {
        let mut vec = Vec::with_capacity(self.apps.len());
        while let Some(item) = self.apps.pop_back() {
            vec.push(item);
        }
        AsyncAppsBase::new(vec)
    }
}

pub struct AsyncAppsBase<E> {
    apps: Vec<(u8, &'static str, Arc<dyn AsyncApp<E>>)>,
    pha: PhantomData<E>,
}

impl<E> AsyncAppsBase<E> {
    pub fn new(apps: Vec<(u8, &'static str, Arc<dyn AsyncApp<E>>)>) -> AsyncAppsBase<E> {
        AsyncAppsBase {
            apps,
            pha: Default::default(),
        }
    }
}

pub trait AsyncApp<E>: Send + Sync + 'static {
    fn on_start(&self, ctx: &ContextHandle<E>) {}

    fn on_packet(&self, packet: PacketReceived, ctx: &ContextHandle<E>) -> Option<PacketReceived> {
        Some(packet)
    }

    fn on_event(&self, event: E, ctx: &ContextHandle<E>) -> Option<E> {
        Some(event)
    }
}

impl<E> P4app<E> for AsyncAppsBase<E>
    where
        E: Event,
{
    fn on_start(self: &mut Self, ctx: &ContextHandle<E>) {
        let mut apps = self.apps.clone();
        let my_ctx: ContextHandle<E> = ctx.clone();
        tokio::spawn(async move {
            for (_, name, app) in apps.iter() {
                app.on_start(&my_ctx);
            }
        });
    }

    fn on_packet(self: &mut Self, packet: PacketReceived, ctx: &ContextHandle<E>) -> Option<PacketReceived> {
        let mut apps = self.apps.clone();
        let my_ctx: ContextHandle<E> = ctx.clone();
        tokio::spawn(async move {
            let mut p = packet;
            for (_, name, app) in apps.iter() {
                if let Some(packet) = app.on_packet(p, &my_ctx) {
                    p = packet;
                } else {
                    break;
                }
            }
        });
        None
    }

    fn on_event(self: &mut Self, event: E, ctx: &ContextHandle<E>) -> Option<E> {
        let mut apps = self.apps.clone();
        let my_ctx: ContextHandle<E> = ctx.clone();
        tokio::spawn(async move {
            let mut e = event;
            for (_, name, app) in apps.iter() {
                if let Some(event) = app.on_event(e, &my_ctx) {
                    e = event;
                } else {
                    break;
                }
            }
        });
        None
    }
}

impl<A, E> AsyncApp<E> for Mutex<A> where A: P4app<E> + Send, E: Event {
    fn on_start(&self, ctx: &ContextHandle<E>) {
        let mut a = self.lock().unwrap();
        a.on_start(ctx);
    }

    fn on_packet(&self, packet: PacketReceived, ctx: &ContextHandle<E>) -> Option<PacketReceived> {
        let mut a = self.lock().unwrap();
        a.on_packet(packet, ctx)
    }

    fn on_event(&self, event: E, ctx: &ContextHandle<E>) -> Option<E> {
        let mut a = self.lock().unwrap();
        a.on_event(event, ctx)
    }
}