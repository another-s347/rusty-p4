use crate::app::P4app;
use crate::context::ContextHandle;
use crate::event::{Event, PacketReceived};
use failure::_core::marker::PhantomData;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

pub struct AsyncAppsBuilder {}

pub struct AsyncAppsBase<E> {
    apps: Vec<(u8, Arc<Box<dyn AsyncApp<E>>>)>,
    pha: PhantomData<E>,
}

pub trait AsyncApp<E>: Send + Sync {
    fn on_start(&self, ctx: &ContextHandle<E>);

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
        let my_ctx:ContextHandle<E> = ctx.clone();
        tokio::spawn(async move {
            for (_,app) in apps.iter() {
                app.on_start(&my_ctx);
            }
        });
    }

    fn on_packet(self: &mut Self, packet: PacketReceived, ctx: &ContextHandle<E>) {
        let mut apps = self.apps.clone();
        let my_ctx:ContextHandle<E> = ctx.clone();
        tokio::spawn(async move {
            let mut p = packet;
            for (_,app) in apps.iter() {
                if let Some(packet) = app.on_packet(p,&my_ctx) {
                    p = packet;
                }
                else {
                    break;
                }
            }
        });
    }

    fn on_event(self: &mut Self, event: E, ctx: &ContextHandle<E>) {
        let mut apps = self.apps.clone();
        let my_ctx:ContextHandle<E> = ctx.clone();
        tokio::spawn(async move {
            let mut e = event;
            for (_,app) in apps.iter() {
                if let Some(event) = app.on_event(e,&my_ctx) {
                    e = event;
                }
                else {
                    break;
                }
            }
        });
    }
}
