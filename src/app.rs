use crate::proto::p4runtime::PacketIn;
use crate::context::ContextHandle;
use futures03::future::Future;
use crate::event::PacketReceived;
use bytes::{Bytes, BytesMut};
use crate::util::packet::packet_in_header::PacketInHeader;
use crate::util::packet::Ethernet;
use crate::util::packet::data::Data;
use crate::util::packet::Packet;
use log::{info, trace, warn, debug, error};
use crate::util::flow::{FlowTable, FlowAction, Flow};
use crate::util::value::{Value, ParamValue};
use std::net::Ipv4Addr;
use std::str::FromStr;

mod netconfig;
mod extended;
mod common;
mod statistic;

pub trait p4App {
    fn on_start(self:&mut Self, ctx:&ContextHandle) {}

    fn on_packet(self:&mut Self, packet:PacketReceived, ctx: &ContextHandle) {}

    fn on_device(self:&mut Self, device:String, ctx:&ContextHandle) {}
}

pub struct Example {
    pub counter:u32
}

impl p4App for Example {
    fn on_packet(self:&mut Self, packet:PacketReceived, ctx: &ContextHandle) {
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

    fn on_device(self:&mut Self, device:String, ctx:&ContextHandle) {
        info!(target:"Example App","device up {}", device);
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
            device,
            table: flow_table,
            action: flow_action,
            priority: 0,
            metadata: 0
        };
        ctx.insert_flow(flow);
    }
}