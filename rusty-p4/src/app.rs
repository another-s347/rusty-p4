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
pub mod proxyarp;
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
