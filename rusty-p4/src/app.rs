use std::net::Ipv4Addr;
use std::str::FromStr;

use crate::app::async_app::ExampleAsyncApp;
use crate::app::sync_app::AsyncWrap;
use crate::context::ContextHandle;
use crate::event::{CommonEvents, Event, PacketReceived};
use crate::proto::p4runtime::PacketIn;
use crate::util::flow::*;
use crate::util::packet::Ethernet;
use crate::util::packet::Packet;
use crate::util::value::EXACT;
use crate::util::value::{encode, LPM};
use bytes::{Bytes, BytesMut};
use futures03::future::Future;
use log::{debug, error, info, trace, warn};
use std::any::Any;
use std::cell::{Ref, RefCell};
use std::ops::Deref;
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};

pub mod async_app;
pub mod common;
//pub mod extended;
pub mod graph;
pub mod linkprobe;
//pub mod proxyarp;
pub mod statistic;
pub mod sync_app;

pub trait P4app<E>: 'static
where
    E: Event,
{
    fn on_start(self: &mut Self, ctx: &ContextHandle<E>) {}

    fn on_packet(
        self: &mut Self,
        packet: PacketReceived,
        ctx: &ContextHandle<E>,
    ) -> Option<PacketReceived> {
        Some(packet)
    }

    fn on_event(self: &mut Self, event: E, ctx: &ContextHandle<E>) -> Option<E> {
        Some(event)
    }
}

pub enum Service<T> {
    Async(Arc<T>),
    AsyncFromSyncWrap(Arc<Mutex<T>>),
    SyncFromAsyncWrap(Rc<RefCell<AsyncWrap<T>>>),
    Sync(Rc<RefCell<T>>),
}

impl<T> Clone for Service<T> {
    fn clone(&self) -> Self {
        match self {
            Service::Async(i) => Service::Async(i.clone()),
            Service::AsyncFromSyncWrap(i) => Service::AsyncFromSyncWrap(i.clone()),
            Service::SyncFromAsyncWrap(i) => Service::SyncFromAsyncWrap(i.clone()),
            Service::Sync(i) => Service::Sync(i.clone()),
        }
    }
}

pub enum DefaultServiceStorage {
    Async(Arc<dyn Any + Send + Sync>),
    AsyncFromSyncWrap(Arc<dyn Any + Send + Sync>),
    SyncFromAsyncWrap(Rc<dyn Any>),
    Sync(Rc<dyn Any>),
}

pub enum ServiceGuard<'a, T> {
    Ref(Ref<'a, T>),
    Direct(&'a T),
    Mutex(MutexGuard<'a, T>),
}

impl<T> Service<T>
where
    T: 'static,
{
    pub fn get(&self) -> ServiceGuard<T> {
        self.try_get().unwrap()
    }

    pub fn try_get(&self) -> Option<ServiceGuard<T>> {
        Some(match self {
            Service::Sync(i) => ServiceGuard::Ref(i.borrow()),
            Service::Async(i) => ServiceGuard::Direct(i),
            Service::AsyncFromSyncWrap(i) => ServiceGuard::Mutex(i.lock().ok()?),
            Service::SyncFromAsyncWrap(i) => ServiceGuard::Ref(Ref::map(i.borrow(), |x| &x.inner)),
        })
    }

    pub fn to_sync_storage(self) -> DefaultServiceStorage {
        match self {
            Service::Sync(i) => DefaultServiceStorage::Sync(i),
            Service::SyncFromAsyncWrap(i) => DefaultServiceStorage::SyncFromAsyncWrap(i),
            _ => unreachable!(),
        }
    }
}

impl<T> Service<T>
where
    T: 'static + Send + Sync,
{
    pub fn to_async_storage(self) -> DefaultServiceStorage {
        match self {
            Service::Async(i) => DefaultServiceStorage::Async(i),
            Service::AsyncFromSyncWrap(i) => DefaultServiceStorage::AsyncFromSyncWrap(i),
            _ => unreachable!(),
        }
    }
}

impl<T> Service<T>
where
    T: 'static + Send,
{
    pub fn to_sync_wrap_storage(self) -> DefaultServiceStorage {
        match self {
            Service::AsyncFromSyncWrap(i) => DefaultServiceStorage::AsyncFromSyncWrap(i),
            _ => unreachable!(),
        }
    }
}

impl<'a, T> Deref for ServiceGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            ServiceGuard::Ref(i) => i,
            ServiceGuard::Direct(i) => i,
            ServiceGuard::Mutex(i) => i,
        }
    }
}

pub trait ServiceStorage<T> {
    fn to_service(self) -> Option<Service<T>>;
}

default impl<T> ServiceStorage<T> for DefaultServiceStorage
where
    T: 'static,
{
    fn to_service(self) -> Option<Service<T>> {
        match self {
            DefaultServiceStorage::Sync(i) => {
                Some(Service::Sync(Rc::downcast::<RefCell<T>>(i).unwrap()))
            }
            DefaultServiceStorage::SyncFromAsyncWrap(i) => Some(Service::SyncFromAsyncWrap(
                Rc::downcast::<RefCell<AsyncWrap<T>>>(i).unwrap(),
            )),
            DefaultServiceStorage::Async(i) => None,
            DefaultServiceStorage::AsyncFromSyncWrap(i) => None,
        }
    }
}

impl<T> ServiceStorage<T> for DefaultServiceStorage
where
    T: Send + Sync + 'static,
{
    fn to_service(self) -> Option<Service<T>> {
        Some(match self {
            DefaultServiceStorage::Sync(i) => Service::Sync(Rc::downcast::<RefCell<T>>(i).unwrap()),
            DefaultServiceStorage::SyncFromAsyncWrap(i) => {
                Service::SyncFromAsyncWrap(Rc::downcast::<RefCell<AsyncWrap<T>>>(i).unwrap())
            }
            DefaultServiceStorage::Async(i) => Service::Async(Arc::downcast::<T>(i).unwrap()),
            DefaultServiceStorage::AsyncFromSyncWrap(i) => {
                Service::AsyncFromSyncWrap(Arc::downcast::<Mutex<T>>(i).unwrap())
            }
        })
    }
}

pub struct Example {
    pub counter: u32,
}

impl Example {
    pub fn test(&self) {
        println!("Example: counter={}", self.counter);
    }
}

impl P4app<CommonEvents> for Example {
    fn on_packet(
        self: &mut Self,
        packet: PacketReceived,
        ctx: &ContextHandle<CommonEvents>,
    ) -> Option<PacketReceived> {
        let parsed: Option<Ethernet<&[u8]>> = Ethernet::from_bytes(packet.get_packet_bytes());
        if let Some(ethernet) = parsed {
            self.counter += 1;
            info!(target:"Example App","Counter == {}, ethernet type == {:x}", self.counter, ethernet.ether_type);
        } else {
            warn!(target:"Example App","packet parse fail");
        }
        None
    }

    fn on_event(
        self: &mut Self,
        event: CommonEvents,
        ctx: &ContextHandle<CommonEvents>,
    ) -> Option<CommonEvents> {
        match event {
            CommonEvents::DeviceAdded(ref device) => {
                info!(target:"Example App","device up {:?}", device);
                let flow = flow! {
                    pipe="MyIngress";
                    table="ipv4_lpm";
                    key={
                        "hdr.ipv4.dstAddr"=>ip"10.0.2.2"/32
                    };
                    action=myTunnel_ingress(dst_id:100u32);
                };
                ctx.insert_flow(flow, device.id);
            }
            _ => {}
        }
        None
    }
}

#[test]
fn test_SyncFromAsyncWrap_storage() {
    let t = ExampleAsyncApp::new();
    let service_t = Service::SyncFromAsyncWrap(Rc::new(RefCell::new(AsyncWrap::new(t))));
    let storage_t = service_t.to_sync_storage();
    let get_t: Service<ExampleAsyncApp> = storage_t.to_service().unwrap();
    get_t.get().test();
}

#[test]
fn test_Async_storage() {
    let t = ExampleAsyncApp::new();
    let service_t = Service::Async(Arc::new(t));
    let storage_t = service_t.to_async_storage();
    let get_t: Service<ExampleAsyncApp> = storage_t.to_service().unwrap();
    get_t.get().test();
}

#[test]
fn test_AsyncFromSyncWrap_storage() {
    let t = Example { counter: 0 };
    let service_t = Service::AsyncFromSyncWrap(Arc::new(Mutex::new(t)));
    let storage_t = service_t.to_async_storage();
    let get_t: Service<Example> = storage_t.to_service().unwrap();
    get_t.get().test();
}

#[test]
fn test_Sync_storage() {
    let t = Example { counter: 0 };
    let service_t = Service::Sync(Rc::new(RefCell::new(t)));
    let storage_t = service_t.to_sync_storage();
    let get_t: Service<Example> = storage_t.to_service().unwrap();
    get_t.get().test();
}
