use crate::context::ContextHandle;
use crate::event::{Event, CoreRequest, CommonEvents};
use std::time::Duration;
use futures::prelude::*;
use futures03::prelude::*;
use log::{info, trace, warn, debug, error};
use crate::util::flow::{Flow, FlowTable, FlowAction};
use crate::util::value::{Value, MAC};
use crate::util::packet::Ethernet;
use crate::util::packet::Packet;
use bytes::Bytes;
use crate::util::packet::data::Data;
use crate::representation::{Device, ConnectPoint, Link, DeviceID};
use crate::app::extended::{P4appInstallable, P4appExtendedCore, EtherPacketHook};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crate::app::common::CommonState;

pub struct LinkProbeLoader {
    inner:Arc<Mutex<HashMap<u16,u16>>>
}

impl LinkProbeLoader {
    pub fn new() -> Self {
        LinkProbeLoader {
            inner: Arc::new(Mutex::new(Default::default()))
        }
    }
}

impl<A,E> P4appInstallable<A,E> for LinkProbeLoader
    where E:Event
{
    fn install(&mut self, extend_core: &mut P4appExtendedCore<A, E>) {
        let my_state = self.inner.clone();
        extend_core.install_ether_hook(0x861,Box::new(on_probe_received));
    }
}

pub fn on_probe_received<E>(data:Data,cp:ConnectPoint,state:&CommonState,ctx:&ContextHandle<E>) where E:Event {
    let probe:Result<ConnectPoint,serde_json::Error> = serde_json::from_slice(&data.0);
    if let Ok(from) = probe {
        let this = cp;
        let from = from.to_owned();
        ctx.send_event(CommonEvents::LinkDetected(Link {
            src: from,
            dst: this
        }).into());
    }
    else {
        error!(target:"linkprobe","invalid probe == {:?}",probe);
    }
}

pub fn on_device_added<E>(device:&Device,ctx:&ContextHandle<E>) where E:Event
{
    new_probe_interceptor(device.id,ctx);
    let mut linkprobe_per_ports = Vec::new();
    for port in device.ports.iter().map(|x|x.number) {
        let cp = ConnectPoint {
            device: device.id,
            port
        };
        let mut my_sender = ctx.sender.clone();
        let probe = new_probe(&cp);
        let mut interval = tokio::timer::Interval::new_interval(Duration::new(3,0));
        let task = tokio::spawn(async move {
            while let Some(s) = interval.next().await {
//                info!(target:"linkprobe","device probe {}", &name);
                my_sender.send(CoreRequest::PacketOut {
                    connect_point:cp,
                    packet: probe.clone()
                }).await.unwrap();
            }
        });
        linkprobe_per_ports.push(task);
    }
}

pub fn new_probe_interceptor<E>(device_id:DeviceID,ctx:&ContextHandle<E>) where E:Event {
    let flow = Flow {
        device: device_id,
        table: FlowTable {
            name: "IngressPipeImpl.acl",
            matches: &[("hdr.ethernet.ether_type",Value::EXACT(0x861u16))]
        },
        action: FlowAction {
            name: "send_to_cpu",
            params: &[]
        },
        priority: 4000
    };
    ctx.insert_flow(flow);
}

pub fn new_probe(cp:&ConnectPoint) -> Bytes
{
    let probe = serde_json::to_vec(cp).unwrap();
    Ethernet {
        src: MAC([0x12,0x34,0x56,0x12,0x34,0x56]),
        dst: MAC::broadcast(),
        ether_type: 0x861,
        payload: Data(Bytes::from(probe))
    }.into_bytes()
}