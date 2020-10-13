use async_trait::async_trait;
use std::net::Ipv4Addr;
use std::str::FromStr;
//use crate::app::async_app::ExampleAsyncApp;
//use crate::app::sync_app::AsyncWrap;
// use crate::core::DefaultContext;
use crate::event::{CommonEvents, Event, NorthboundRequest, PacketReceived};
use crate::proto::p4runtime::PacketIn;
use crate::util::flow::*;
use crate::util::packet::Ethernet;
use crate::util::packet::Packet;
use crate::util::value::EXACT;
use crate::util::value::{encode, LPM};
use bytes::{Bytes, BytesMut};
use futures::{FutureExt, future::{BoxFuture, Future}};
use log::{debug, error, info, trace, warn};
use std::any::{TypeId, Any};
use std::cell::{Ref, RefCell};
use std::ops::Deref;
use std::rc::Rc;
use std::{collections::HashMap, sync::{Arc, Mutex, MutexGuard}};
// use crate::core::context::Context;
use tuple_list::TupleList;
use tuple_list::tuple_list_type;

pub mod common;
pub mod graph;
pub mod device_manager;
// pub mod rest;
// pub mod statistic;
// pub mod raw_statistic;
// pub mod stratum_statistic;
// pub mod app_service;
pub mod options;
pub mod store;
pub mod default;

#[async_trait]
pub trait App: Sync + Send + 'static {
    type Dependency: Dependencies;
    type Option: options::AppOption;

    fn init<S>(dependencies: Self::Dependency, store: &mut S, option: Self::Option) -> Self where S: store::AppStore;

    async fn run(&self);
}

#[async_trait]
impl<T> App for Option<T> where T: App {
    type Dependency = T::Dependency;

    type Option = T::Option;

    fn init<S>(dependencies: Self::Dependency, store: &mut S, option: Self::Option) -> Self where S: store::AppStore  {
        todo!()
    }

    async fn run(&self) {
        if let Some(s) = self {
            s.run().await;
        };
    }
}

#[async_trait]
impl<T> App for Arc<T> where T: App {
    type Dependency = T::Dependency;

    type Option = T::Option;

    fn init<S>(dependencies: Self::Dependency, store: &mut S, option: Self::Option) -> Self where S: store::AppStore  {
        todo!()
    }

    async fn run(&self) {
        self.as_ref().run().await;
    }
}

pub trait Dependencies {
    fn get<S>(store: &mut S) -> Self where S: store::AppStore;
}

impl<Head, Tail> Dependencies for (Head, Tail) where
    Head: App + Clone,
    Tail: Dependencies + TupleList + Clone,
{
    fn get<S>(store: &mut S) -> Self 
    where S: store::AppStore 
    {
        let a:Head = if let Some(a) = store.get() {
            a
        } else {
            todo!()
        };

        let b:Tail = Tail::get(store);

        return (a,b);
    }
}

impl Dependencies for () {
    fn get<S>(store: &mut S) -> Self where S: store::AppStore {
        ()
    }
}

// #[async_trait]
// pub trait P4app<E, C>: 'static + Send
// where
//     E: Event,
//     C: Context<E>
// {
//     async fn on_start(self: &mut Self, ctx: &mut C) {}

//     async fn on_packet(
//         self: &mut Self,
//         packet: PacketReceived,
//         ctx: &mut C,
//     ) -> Option<PacketReceived> {
//         Some(packet)
//     }

//     async fn on_event(self: &mut Self, event: E, ctx: &mut C) -> Option<E> {
//         Some(event)
//     }

//     async fn on_request(self: &mut Self, request: NorthboundRequest, ctx: &mut C) {}

//     async fn on_context_update(self: &mut Self, ctx: &mut C) {}
// }

// pub struct Example {
//     pub counter: u32,
// }

// impl Example {
//     pub fn test(&self) {
//         println!("Example: counter={}", self.counter);
//     }
// }

// #[async_trait]
// impl<C> P4app<CommonEvents, C> for Example where C: Context<CommonEvents>{
//     async fn on_packet(
//         self: &mut Self,
//         packet: PacketReceived,
//         ctx: &mut C,
//     ) -> Option<PacketReceived> {
//         let parsed: Option<Ethernet<&[u8]>> = Ethernet::from_bytes(packet.packet.as_slice());
//         if let Some(ethernet) = parsed {
//             self.counter += 1;
//             info!(target:"Example App","Counter == {}, ethernet type == {:x}", self.counter, ethernet.ether_type);
//         } else {
//             warn!(target:"Example App","packet parse fail");
//         }
//         None
//     }

//     async fn on_event(
//         self: &mut Self,
//         event: CommonEvents,
//         ctx: &mut C,
//     ) -> Option<CommonEvents> {
//         match event {
//             CommonEvents::DeviceAdded(ref device) => {
//                 info!(target:"Example App","device up {:?}", device);
//                 //                let flow = flow! {
//                 //                    pipe:"MyIngress",
//                 //                    table:"ipv4_lpm" {
//                 //                        "hdr.ipv4.dstAddr"=>ipv4!(10.0.2.2)/32
//                 //                    }
//                 //                    action:"myTunnel_ingress"{
//                 //                        dst_id:100u32
//                 //                    }
//                 //                };
//                 //                ctx.insert_flow(flow, device.id);
//             }
//             _ => {}
//         }
//         None
//     }
// }
