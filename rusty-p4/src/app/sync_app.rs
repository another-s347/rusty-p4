use crate::app::async_app::AsyncApp;
use crate::app::P4app;
use crate::context::ContextHandle;
use crate::event::{Event, PacketReceived};
use failure::_core::marker::PhantomData;
use std::collections::LinkedList;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub struct SyncAppsBuilder<E> {
    apps: LinkedList<(u8, &'static str, Box<dyn P4app<E>>)>,
}

impl<E> SyncAppsBuilder<E>
where
    E: Event,
{
    pub fn with_async<T>(&mut self, mut priority: u8, name: &'static str, app: T)
    where
        T: AsyncApp<E>,
    {
        let mut iter = self.apps.iter_mut();
        while let Some((p, name, _)) = iter.next() {
            if priority > *p {
                iter.insert_next((priority, name, Box::new(AsyncWrap::new(app))));
                break;
            } else if priority == *p {
                priority -= 1;
                break;
            }
        }
    }

    pub fn with<T>(&mut self, mut priority: u8, name: &'static str, app: T)
    where
        T: P4app<E>,
    {
        let mut iter = self.apps.iter_mut();
        while let Some((p, name, _)) = iter.next() {
            if priority > *p {
                iter.insert_next((priority, name, Box::new(app)));
                break;
            } else if priority == *p {
                priority -= 1;
                break;
            }
        }
    }

    pub fn build(mut self) -> SyncAppsBase<E> {
        let mut vec = Vec::with_capacity(self.apps.len());
        while let Some(item) = self.apps.pop_back() {
            vec.push(item);
        }
        SyncAppsBase::new(vec)
    }
}

pub struct SyncAppsBase<E> {
    apps: Vec<(u8, &'static str, Box<dyn P4app<E>>)>,
    pha: PhantomData<E>,
}

impl<E> SyncAppsBase<E> {
    pub fn new(apps: Vec<(u8, &'static str, Box<dyn P4app<E>>)>) -> SyncAppsBase<E> {
        SyncAppsBase {
            apps,
            pha: Default::default(),
        }
    }
}

impl<E> P4app<E> for SyncAppsBase<E>
where
    E: Event,
{
    fn on_start(self: &mut Self, ctx: &ContextHandle<E>) {
        let my_ctx: ContextHandle<E> = ctx.clone();
        for (_, name, app) in self.apps.iter_mut() {
            app.on_start(&my_ctx);
        }
    }

    fn on_packet(
        self: &mut Self,
        packet: PacketReceived,
        ctx: &ContextHandle<E>,
    ) -> Option<PacketReceived> {
        let my_ctx: ContextHandle<E> = ctx.clone();
        let mut p = packet;
        for (_, name, app) in self.apps.iter_mut() {
            if let Some(packet) = app.on_packet(p, &my_ctx) {
                p = packet;
            } else {
                break;
            }
        }
        None
    }

    fn on_event(self: &mut Self, event: E, ctx: &ContextHandle<E>) -> Option<E> {
        let my_ctx: ContextHandle<E> = ctx.clone();
        let mut e = event;
        for (_, name, app) in self.apps.iter_mut() {
            if let Some(event) = app.on_event(e, &my_ctx) {
                e = event;
            } else {
                break;
            }
        }
        None
    }
}

struct AsyncWrap<A> {
    inner: A,
}

impl<A, E> P4app<E> for AsyncWrap<A>
where
    A: AsyncApp<E>,
    E: Event,
{
    fn on_start(&mut self, ctx: &ContextHandle<E>) {
        self.inner.on_start(ctx);
    }

    fn on_packet(
        &mut self,
        packet: PacketReceived,
        ctx: &ContextHandle<E>,
    ) -> Option<PacketReceived> {
        self.inner.on_packet(packet, ctx)
    }

    fn on_event(&mut self, event: E, ctx: &ContextHandle<E>) -> Option<E> {
        self.inner.on_event(event, ctx)
    }
}

impl<T> AsyncWrap<T> {
    pub fn new<E>(app: T) -> AsyncWrap<T>
    where
        T: AsyncApp<E>,
        E: Event,
    {
        AsyncWrap { inner: app }
    }
}
