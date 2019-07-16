use std::net::Ipv4Addr;
use std::str::FromStr;

use bytes::{Bytes, BytesMut};
use futures03::future::Future;
use log::{debug, error, info, trace, warn};

use crate::context::ContextHandle;
use crate::event::{PacketReceived, Event, CommonEvents};
use crate::proto::p4runtime::PacketIn;
use crate::util::flow::{Flow, FlowAction, FlowTable};
use crate::util::packet::data::Data;
use crate::util::packet::Ethernet;
use crate::util::packet::Packet;
use crate::util::packet::packet_in_header::PacketInHeader;
use crate::util::value::{ParamValue, Value};

pub mod extended;
pub mod common;
pub mod statistic;
pub mod graph;
pub mod linkprobe;
pub mod proxyarp;

pub trait p4App<E>
    where E:Event
{
    fn on_start(self:&mut Self, ctx:&ContextHandle<E>) {}

    fn on_packet(self:&mut Self, packet:PacketReceived, ctx: &ContextHandle<E>) {}

    fn on_event(self:&mut Self, event:E, ctx:&ContextHandle<E>) {}
}

pub struct Example {
    pub counter:u32
}

impl p4App<CommonEvents> for Example {
    fn on_packet(self:&mut Self, packet:PacketReceived, ctx: &ContextHandle<CommonEvents>) {
        let packet = BytesMut::from(packet.packet.payload);
        let parsed:Option<Ethernet<Data>> = Ethernet::from_bytes(packet);
        if let Some(ethernet) = parsed {
            self.counter+=1;
            info!(target:"Example App","Counter == {}, ethernet type == {:x}", self.counter, ethernet.ether_type);
        }
        else {
            warn!(target:"Example App","packet parse fail");
        }
    }

    fn on_event(self:&mut Self, event:CommonEvents, ctx:&ContextHandle<CommonEvents>) {
        match event {
            CommonEvents::DeviceAdded(ref device)=>{
                info!(target:"Example App","device up {:?}", device);
                let flow_table = FlowTable {
                    name: "MyIngress.ipv4_lpm",
                    matches: &[
                        ("hdr.ipv4.dstAddr", Value::LPM(Ipv4Addr::from_str("10.0.2.2").unwrap(), 32))
                    ]
                };
                let flow_action = FlowAction {
                    name: "MyIngress.myTunnel_ingress",
                    params: &[
                        ("dst_id", ParamValue::of(100u32))
                    ]
                };
                let flow = Flow {
                    device:device.name.to_string(),
                    table: flow_table,
                    action: flow_action,
                    priority: 0,
                    metadata: 0
                };
                ctx.insert_flow(flow);
            }
            _=>{}
        }
    }
}