use std::net::Ipv4Addr;
use std::str::FromStr;

use crate::context::ContextHandle;
use crate::event::{CommonEvents, Event, PacketReceived};
use crate::proto::p4runtime::PacketIn;
use crate::util::flow::*;
use crate::util::packet::data::Data;
use crate::util::packet::Ethernet;
use crate::util::packet::Packet;
use crate::util::value::EXACT;
use crate::util::value::{encode, LPM};
use bytes::{Bytes, BytesMut};
use futures03::future::Future;
use log::{debug, error, info, trace, warn};

pub mod async_app;
pub mod common;
pub mod extended;
pub mod graph;
pub mod linkprobe;
pub mod proxyarp;
pub mod statistic;

pub trait P4app<E>: Send + 'static
where
    E: Event,
{
    fn on_start(self: &mut Self, ctx: &ContextHandle<E>) {}

    fn on_packet(self: &mut Self, packet: PacketReceived, ctx: &ContextHandle<E>) {}

    fn on_event(self: &mut Self, event: E, ctx: &ContextHandle<E>) {}
}

pub struct Example {
    pub counter: u32,
}

impl P4app<CommonEvents> for Example {
    fn on_packet(self: &mut Self, packet: PacketReceived, ctx: &ContextHandle<CommonEvents>) {
        let packet = BytesMut::from(packet.packet.payload);
        let parsed: Option<Ethernet<Data>> = Ethernet::from_bytes(packet);
        if let Some(ethernet) = parsed {
            self.counter += 1;
            info!(target:"Example App","Counter == {}, ethernet type == {:x}", self.counter, ethernet.ether_type);
        } else {
            warn!(target:"Example App","packet parse fail");
        }
    }

    fn on_event(self: &mut Self, event: CommonEvents, ctx: &ContextHandle<CommonEvents>) {
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
    }
}
