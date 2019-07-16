use crate::context::ContextHandle;
use crate::event::{Event, CoreRequest, CommonEvents};
use std::time::Duration;
use futures::prelude::*;
use futures03::prelude::*;
use log::{info, trace, warn, debug, error};
use crate::util::flow::{Flow, FlowTable, FlowAction};
use crate::util::value::{Value, MAC};
use crate::util::packet::{Ethernet, Arp, arp::{
    ArpOp
}};
use crate::util::packet::Packet;
use bytes::Bytes;
use crate::util::packet::data::Data;
use crate::util::packet::arp::ETHERNET_TYPE_ARP;
use crate::representation::{Device, ConnectPoint, Host};
use crate::app::common::CommonState;

pub fn on_arp_received<E>(device:&Device, port:u32, data:Data,state:&CommonState,ctx:&ContextHandle<E>) where E:Event {
    let arp = Arp::from_bytes(data.0.into());
    if arp.is_none() {
        error!(target:"proxyarp","invalid arp packet");
        return;
    }
    let arp = arp.unwrap();
    match arp.opcode {
        ArpOp::Request => {
            let cp = ConnectPoint {
                device: device.name.clone(),
                port
            };
            let host = Host {
                mac: arp.sender_mac,
                ip: arp.sender_ip.into(),
                location: cp
            };
            if !state.hosts.contains(&host) {
                ctx.send_event(CommonEvents::HostDetected(host).into());
            }
            if let Some(arp_target) = state.hosts.iter().find(|x|x.ip==arp.target_ip) {
                let arp_reply = Arp {
                    hw_type: 1,
                    proto_type: 0x800,
                    hw_addr_len: 6,
                    proto_addr_len: 4,
                    opcode: ArpOp::Reply,
                    sender_mac: arp_target.mac,
                    sender_ip: arp_target.ip,
                    target_mac: arp.sender_mac,
                    target_ip: arp.sender_ip
                };
                let packet = Ethernet {
                    src: arp_target.mac,
                    dst: arp.sender_mac,
                    ether_type: 0x806,
                    payload: arp_reply
                }.into_bytes();
                ctx.sender.unbounded_send(CoreRequest::PacketOut {
                    device: device.name.clone(),
                    port,
                    packet
                }).unwrap();
            }
        }
        ArpOp::Reply=> {
            let cp = ConnectPoint {
                device: device.name.clone(),
                port
            };
            let host = Host {
                mac: arp.sender_mac,
                ip: arp.sender_ip.into(),
                location: cp
            };
            ctx.send_event(CommonEvents::HostDetected(host).into());
        }
        ArpOp::Unknown(op)=>{
            error!(target:"proxyarp","unknown arp op code: {}", op);
        }
    }
}

pub fn on_device_added<E>(device:&Device,ctx:&ContextHandle<E>) where E:Event
{
    new_arp_interceptor(&device.name,ctx);
}

pub fn new_arp_interceptor<E>(device_name:&str,ctx:&ContextHandle<E>) where E:Event {
    let flow = Flow {
        device: device_name.to_owned(),
        table: FlowTable {
            name: "IngressPipeImpl.acl",
            matches: &[("hdr.ethernet.ether_type",Value::EXACT(ETHERNET_TYPE_ARP))]
        },
        action: FlowAction {
            name: "send_to_cpu",
            params: &[]
        },
        priority: 4000,
        metadata: 0
    };
    ctx.insert_flow(flow);
}