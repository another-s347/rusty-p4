use bytes::Bytes;
use futures::prelude::*;
use log::{debug, error, info, trace, warn};
use rusty_p4::app::common::CommonState;
use rusty_p4::app::P4app;
use rusty_p4::event::{CommonEvents, CoreRequest, Event, PacketReceived};
use rusty_p4::p4rt::pipeconf::PipeconfID;
use rusty_p4::representation::{ConnectPoint, Device, DeviceID, DeviceType, Host};
use rusty_p4::service::{Service};
use rusty_p4::util::flow::*;
use rusty_p4::util::packet::arp::ETHERNET_TYPE_ARP;
use rusty_p4::util::packet::Packet;
use rusty_p4::util::packet::{arp::ArpOp, Arp, Ethernet};
use rusty_p4::util::value::{Value, EXACT, MAC};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use rusty_p4::core::context::Context;

#[derive(Clone)]
pub struct ProxyArpState {
    pub interceptor: Arc<HashMap<PipeconfID, Box<dyn ArpInterceptor>>>,
    pub commonstate_service: CommonState,
}

pub struct ProxyArpLoader {
    interceptor: HashMap<PipeconfID, Box<dyn ArpInterceptor>>,
}

pub trait ArpInterceptor: Sync + Send {
    fn new_flow(&self, device: DeviceID) -> Flow;
}

impl ProxyArpLoader {
    pub fn new() -> Self {
        ProxyArpLoader {
            interceptor: Default::default(),
        }
    }

    pub fn with_interceptor<T: 'static>(mut self, pipeconf: &str, interceptor: T) -> Self
    where
        T: ArpInterceptor,
    {
        let pipeconf = rusty_p4::util::hash(pipeconf);
        self.interceptor
            .insert(PipeconfID(pipeconf), Box::new(interceptor));
        self
    }

    pub fn build(self, commonstate_service: CommonState) -> ProxyArpState {
        ProxyArpState {
            interceptor: Arc::new(self.interceptor),
            commonstate_service,
        }
    }
}

#[async_trait]
impl<E, C> P4app<E, C> for ProxyArpState
where
    E: Event,
    C: Context<E>
{
    async fn on_packet(
        self: &mut Self,
        packet: PacketReceived,
        ctx: &mut C,
    ) -> Option<PacketReceived> {
        match Ethernet::<&[u8]>::from_bytes(&packet.packet) {
            Some(ethernet) if ethernet.ether_type == 0x806 => {
                let from = ctx.get_connectpoint(&packet).unwrap();
                on_arp_received(ethernet, from, &self.commonstate_service, ctx);
                return None;
            }
            _ => {}
        }
        Some(packet)
    }

    async fn on_event(self: &mut Self, event: E, ctx: &mut C) -> Option<E> {
        match event.try_to_common() {
            Some(CommonEvents::DeviceAdded(device)) => {
                let pipeconf = match &device.typ {
                    DeviceType::Bmv2MASTER {
                        socket_addr,
                        device_id,
                        pipeconf,
                    } => pipeconf,
                    DeviceType::StratumMASTER {
                        socket_addr,
                        device_id,
                        pipeconf,
                    } => pipeconf,
                    _ => {
                        warn!(target:"linkprobe","It is not a master device. link probe may not work.");
                        return Some(event);
                    }
                };
                let interceptor = if let Some(interceptor) = self.interceptor.get(pipeconf) {
                    interceptor
                }
                else {
                    warn!(target:"linkprobe","Pipeconf interceptor not found. link probe may not work.");
                    return Some(event);
                };
                let flow = interceptor.new_flow(device.id);
                ctx.insert_flow(flow, device.id);
            }
            _ => {}
        }
        Some(event)
    }
}

pub fn on_arp_received<E, C>(
    data: Ethernet<&[u8]>,
    cp: ConnectPoint,
    state: &CommonState,
    ctx: &mut C,
) where
    E: Event,
    C: Context<E>
{
    let device = cp.device;
    let data = data.payload;
    let arp = Arp::from_bytes(data);
    if arp.is_none() {
        error!(target:"proxyarp","invalid arp packet");
        return;
    }
    let state = state.inner.lock();
    let arp = arp.unwrap();
    let arp_sender_mac = MAC::from_slice(arp.sender_mac);
    match arp.opcode {
        ArpOp::Request => {
            let host = Host {
                mac: arp.get_sender_mac(),
                ip: Some(arp.get_sender_ipv4().unwrap().into()),
                location: cp,
            };
            if !state.hosts.contains(&host) {
                ctx.send_event(CommonEvents::HostDetected(host).into_e());
            }
            if let Some(arp_target) = state
                .hosts
                .iter()
                .find(|x| x.ip == Some(arp.get_target_ipv4().unwrap().into()))
            {
                let arp_reply = Arp {
                    hw_type: 1,
                    proto_type: 0x800,
                    hw_addr_len: 6,
                    proto_addr_len: 4,
                    opcode: ArpOp::Reply,
                    sender_mac: arp_target.mac.as_ref(),
                    sender_ip: &arp_target.get_ipv4_address().unwrap().octets(),
                    target_mac: arp_sender_mac.as_ref(),
                    target_ip: arp.sender_ip,
                };
                let packet = Ethernet {
                    src: arp_target.mac.as_ref(),
                    dst: arp_sender_mac.as_ref(),
                    ether_type: 0x806,
                    payload: arp_reply,
                }
                .write_to_bytes();
                ctx.send_packet(cp, packet);
            } else {
                let packet = Ethernet {
                    src: arp_sender_mac.as_ref(),
                    dst: MAC::broadcast().as_ref(),
                    ether_type: 0x806,
                    payload: data,
                }
                .write_to_bytes();
                state
                    .devices
                    .iter()
                    .filter(|(_, d)| d.typ.is_master())
                    .for_each(|(_, d)| {
                        for x in &d.ports {
                            if x.number == cp.port {
                                continue;
                            }
                            ctx.send_packet(
                                ConnectPoint {
                                    device: d.id,
                                    port: x.number,
                                },
                                packet.clone(),
                            );
                        }
                    });
            }
        }
        ArpOp::Reply => {
            let host = Host {
                mac: arp_sender_mac,
                ip: Some(arp.get_sender_ipv4().unwrap().into()),
                location: cp,
            };
            ctx.send_event(CommonEvents::HostDetected(host).into_e());
        }
        ArpOp::Unknown(op) => {
            error!(target:"proxyarp","unknown arp op code: {}", op);
        }
    }
}
