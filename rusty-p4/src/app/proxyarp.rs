use crate::app::common::CommonState;
use crate::app::extended::{P4appExtendedCore, P4appInstallable};
use crate::context::ContextHandle;
use crate::event::{CommonEvents, CoreRequest, Event};
use crate::p4rt::pipeconf::PipeconfID;
use crate::representation::{ConnectPoint, Device, DeviceID, DeviceType, Host};
use crate::util::flow::*;
use crate::util::packet::arp::ETHERNET_TYPE_ARP;
use crate::util::packet::data::Data;
use crate::util::packet::Packet;
use crate::util::packet::{arp::ArpOp, Arp, Ethernet};
use crate::util::value::{Value, EXACT, MAC};
use bytes::Bytes;
use futures::prelude::*;
use futures03::prelude::*;
use log::{debug, error, info, trace, warn};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct ProxyArpState {
    pub interceptor: Arc<HashMap<PipeconfID, Box<dyn ArpInterceptor>>>,
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
        let pipeconf = crate::util::hash(pipeconf);
        self.interceptor
            .insert(PipeconfID(pipeconf), Box::new(interceptor));
        self
    }

    pub fn build(self) -> ProxyArpState {
        ProxyArpState {
            interceptor: Arc::new(self.interceptor),
        }
    }
}

impl<A, E> P4appInstallable<A, E> for ProxyArpState
where
    E: Event,
{
    fn install(&mut self, extend_core: &mut P4appExtendedCore<A, E>) {
        let s = self.clone();
        extend_core.install_ether_hook(0x806, Box::new(on_arp_received));
        extend_core.install_device_added_hook(
            "proxy arp",
            Box::new(move |device, state, ctx| {
                let s = s.clone();
                on_device_added(s, device, state, ctx)
            }),
        );
    }
}

pub fn on_arp_received<E>(
    data: Ethernet<Data>,
    cp: ConnectPoint,
    state: &CommonState,
    ctx: &ContextHandle<E>,
) where
    E: Event,
{
    let device = cp.device;
    let data = data.payload;
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
                ip: Some(arp.sender_ip.into()),
                location: cp,
            };
            if !state.hosts.contains(&host) {
                ctx.send_event(CommonEvents::HostDetected(host));
            }
            if let Some(arp_target) = state
                .hosts
                .iter()
                .find(|x| x.ip == Some(arp.target_ip.into()))
            {
                let arp_reply = Arp {
                    hw_type: 1,
                    proto_type: 0x800,
                    hw_addr_len: 6,
                    proto_addr_len: 4,
                    opcode: ArpOp::Reply,
                    sender_mac: arp_target.mac,
                    sender_ip: arp_target.get_ipv4_address().unwrap(),
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
                    .filter(|(_, d)| d.typ.is_master())
                    .for_each(|(_, d)| {
                        for x in &d.ports {
                            if x.number == cp.port {
                                continue;
                            }
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
                ip: Some(arp.sender_ip.into()),
                location: cp,
            };
            ctx.send_event(CommonEvents::HostDetected(host));
        }
        ArpOp::Unknown(op) => {
            error!(target:"proxyarp","unknown arp op code: {}", op);
        }
    }
}

pub fn on_device_added<E>(
    proxyarp_state: ProxyArpState,
    device: &Device,
    state: &CommonState,
    ctx: &ContextHandle<E>,
) where
    E: Event,
{
    let interceptor = match &device.typ {
        DeviceType::MASTER {
            socket_addr,
            device_id,
            pipeconf,
        } => {
            if let Some(interceptor) = proxyarp_state.interceptor.get(pipeconf) {
                interceptor
            } else {
                return;
            }
        }
        _ => {
            warn!(target:"linkprobe","It is not a master device. Proxy arp may not work.");
            return;
        }
    };
    let flow = interceptor.new_flow(device.id);
    ctx.insert_flow(flow, device.id);
}

pub fn new_arp_interceptor<E>(device_id: DeviceID, ctx: &ContextHandle<E>)
where
    E: Event,
{
    let flow = flow! {
        pipe="IngressPipeImpl";
        table="acl";
        key={
            "hdr.ethernet.ether_type"=>ETHERNET_TYPE_ARP
        };
        action=send_to_cpu();
        priority=4000;
    };
    ctx.insert_flow(flow, device_id);
}
