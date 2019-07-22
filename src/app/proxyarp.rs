use crate::app::common::CommonState;
use crate::app::extended::{P4appExtendedCore, P4appInstallable};
use crate::context::ContextHandle;
use crate::event::{CommonEvents, CoreRequest, Event};
use crate::representation::{ConnectPoint, Device, DeviceID, Host};
use crate::util::flow::{Flow, FlowAction, FlowTable};
use crate::util::packet::arp::ETHERNET_TYPE_ARP;
use crate::util::packet::data::Data;
use crate::util::packet::Packet;
use crate::util::packet::{arp::ArpOp, Arp, Ethernet};
use crate::util::value::{Value, MAC};
use bytes::Bytes;
use futures::prelude::*;
use futures03::prelude::*;
use log::{debug, error, info, trace, warn};
use std::time::Duration;

pub struct ProxyArpLoader {}

impl ProxyArpLoader {
    pub fn new() -> Self {
        ProxyArpLoader {}
    }
}

impl<A, E> P4appInstallable<A, E> for ProxyArpLoader
where
    E: Event,
{
    fn install(&mut self, extend_core: &mut P4appExtendedCore<A, E>) {
        extend_core.install_ether_hook(0x806, Box::new(on_arp_received));
        extend_core.install_device_added_hook("proxy arp", Box::new(on_device_added));
    }
}

pub fn on_arp_received<E>(data: Data, cp: ConnectPoint, state: &CommonState, ctx: &ContextHandle<E>)
where
    E: Event,
{
    let device = cp.device;
    let arp = Arp::from_bytes(data.clone().0.into());
    if arp.is_none() {
        error!(target:"proxyarp","invalid arp packet");
        return;
    }
    let arp = arp.unwrap();
    match arp.opcode {
        ArpOp::Request => {
            let host = Host {
                mac: arp.sender_mac,
                ip: arp.sender_ip.into(),
                location: cp,
            };
            if !state.hosts.contains(&host) {
                ctx.send_event(CommonEvents::HostDetected(host).into());
            }
            if let Some(arp_target) = state.hosts.iter().find(|x| x.ip == arp.target_ip) {
                let arp_reply = Arp {
                    hw_type: 1,
                    proto_type: 0x800,
                    hw_addr_len: 6,
                    proto_addr_len: 4,
                    opcode: ArpOp::Reply,
                    sender_mac: arp_target.mac,
                    sender_ip: arp_target.ip,
                    target_mac: arp.sender_mac,
                    target_ip: arp.sender_ip,
                };
                let packet = Ethernet {
                    src: arp_target.mac,
                    dst: arp.sender_mac,
                    ether_type: 0x806,
                    payload: arp_reply,
                }
                .into_bytes();
                ctx.sender
                    .unbounded_send(CoreRequest::PacketOut {
                        connect_point: cp,
                        packet,
                    })
                    .unwrap();
            } else {
                let packet = Ethernet {
                    src: arp.sender_mac,
                    dst: MAC::broadcast(),
                    ether_type: 0x806,
                    payload: data,
                }
                .into_bytes();
                state
                    .devices
                    .iter()
                    .filter(|(&p, _)| p != device)
                    .for_each(|(_, d)| {
                        for x in &d.ports {
                            ctx.sender
                                .unbounded_send(CoreRequest::PacketOut {
                                    connect_point: ConnectPoint {
                                        device: d.id,
                                        port: x.number,
                                    },
                                    packet: packet.clone(),
                                })
                                .unwrap();
                        }
                    });
            }
        }
        ArpOp::Reply => {
            let host = Host {
                mac: arp.sender_mac,
                ip: arp.sender_ip.into(),
                location: cp,
            };
            ctx.send_event(CommonEvents::HostDetected(host).into());
        }
        ArpOp::Unknown(op) => {
            error!(target:"proxyarp","unknown arp op code: {}", op);
        }
    }
}

pub fn on_device_added<E>(device: &Device, state: &CommonState, ctx: &ContextHandle<E>)
where
    E: Event,
{
    new_arp_interceptor(device.id, ctx);
}

pub fn new_arp_interceptor<E>(device_id: DeviceID, ctx: &ContextHandle<E>)
where
    E: Event,
{
    let flow = Flow {
        device: device_id,
        table: FlowTable {
            name: "IngressPipeImpl.acl",
            matches: &[("hdr.ethernet.ether_type", Value::EXACT(ETHERNET_TYPE_ARP))],
        },
        action: FlowAction {
            name: "send_to_cpu",
            params: &[],
        },
        priority: 4000,
    };
    ctx.insert_flow(flow);
}
