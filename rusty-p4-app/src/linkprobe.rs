use rusty_p4::event::{Event, CoreRequest, CommonEvents, PacketReceived};
use std::time::Duration;
use futures::prelude::*;
use log::{info, trace, warn, debug, error};
use rusty_p4::util::flow::*;
use rusty_p4::util::value::{Value, MAC, EXACT};
use rusty_p4::util::packet::Ethernet;
use rusty_p4::util::packet::Packet;
use bytes::Bytes;
use rusty_p4::representation::{Device, ConnectPoint, Link, DeviceID};
//use rusty_p4::app::extended::{P4appInstallable, P4appExtendedCore, EtherPacketHook};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use rusty_p4::app::common::CommonState;
use rusty_p4::representation::DeviceType;
//use futures::prelude::*;
use tokio::sync::oneshot::Sender;
use rusty_p4::p4rt::pipeconf::{Pipeconf, PipeconfID};
use std::any::Any;
use rusty_p4::util::flow::Flow;
use rusty_p4::app::P4app;
use async_trait::async_trait;
use rusty_p4::core::context::Context;

pub struct LinkProbeLoader {
    interceptor:HashMap<PipeconfID, Box<dyn LinkProbeInterceptor>>
}

#[derive(Clone)]
pub struct LinkProbeState {
    pub inner:Arc<Mutex<HashMap<DeviceID,Vec<Sender<()>>>>>,
    pub interceptor:Arc<HashMap<PipeconfID, Box<dyn LinkProbeInterceptor>>>
}

pub trait LinkProbeInterceptor: Sync + Send {
    fn new_flow(&self,device:DeviceID) -> Flow;
}

impl LinkProbeLoader {
    pub fn new() -> Self {
        LinkProbeLoader {
            interceptor: HashMap::new()
        }
    }

    pub fn with_interceptor<T:'static>(mut self,pipeconf:&str,interceptor:T) -> Self where T:LinkProbeInterceptor {
        let pipeconf = rusty_p4::util::hash(pipeconf);
        self.interceptor.insert(PipeconfID(pipeconf),Box::new(interceptor));
        self
    }

    pub fn build(self) -> LinkProbeState {
        LinkProbeState {
            inner: Arc::new(Mutex::new(Default::default())),
            interceptor: Arc::new(self.interceptor)
        }
    }
}

#[async_trait]
impl<E, C> P4app<E, C> for LinkProbeState where E:Event,C:Context<E> {
    async fn on_packet(&mut self, packet: PacketReceived, ctx: &mut C) -> Option<PacketReceived> {
        match Ethernet::<&[u8]>::from_bytes(&packet.packet) {
            Some(ref ethernet) if ethernet.ether_type==0x861 => {
                let probe:Result<ConnectPoint,serde_json::Error> = serde_json::from_slice(&ethernet.payload);
                if let Ok(from) = probe {
                    let this = ctx.get_connectpoint(&packet).unwrap();
                    let from = from.to_owned();
                    ctx.send_event(CommonEvents::LinkDetected(Link {
                        src: from,
                        dst: this
                    }).into_e());
                    return None;
                }
                else {
                    error!(target:"linkprobe","invalid probe == {:?}",probe);
                }
            }
            _ => {}
        }
        Some(packet)
    }

    async fn on_event(&mut self, event: E, ctx: &mut C) -> Option<E> {
        match event.try_to_common() {
            Some(CommonEvents::DeviceAdded(device)) => {
                on_device_added(self,device,ctx);
            }
            Some(CommonEvents::DeviceLost(device)) => {
                let mut s = self.inner.lock().unwrap();
                if let Some(list) = s.remove(&device) {
                    info!(target:"extend","cancel link probe task for device: {:?}",device);
                    for x in list {
                        x.send(());
                    }
                }
            }
            _=>{}
        }
        Some(event)
    }
}

pub fn on_device_added<E, C>(linkprobe_state:&LinkProbeState,device:&Device, ctx:&mut C) where E:Event,C:Context<E>
{
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
            return;
        }
    };
    let interceptor = if let Some(interceptor) = linkprobe_state.interceptor.get(pipeconf) {
        interceptor
    }
    else {
        warn!(target:"linkprobe","Pipeconf interceptor not found. link probe may not work.");
        return;
    };
    let flow = interceptor.new_flow(device.id);
    ctx.insert_flow(flow,device.id);
    let mut linkprobe_per_ports = Vec::new();
    for port in device.ports.iter().map(|x|x.number) {
        let cp = ConnectPoint {
            device: device.id,
            port
        };
        let mut my_sender = ctx.clone();
        let probe = new_probe(&cp);
        let mut interval = tokio::time::interval(Duration::new(3,0));
        let (cancel,mut cancel_r) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            while let Some(s) = interval.next().await {
                if cancel_r.try_recv().is_ok() {
                    break;
                }
                my_sender.send_packet(cp, probe.clone());
            }
        });
        linkprobe_per_ports.push(cancel);
    }
    if !linkprobe_per_ports.is_empty() {
        info!(target:"linkprobe","start probe for device: {:?}",device.id);
    }
    let mut tasks = linkprobe_state.inner.lock().unwrap();
    tasks.insert(device.id,linkprobe_per_ports);
}

pub fn new_probe(cp:&ConnectPoint) -> Bytes
{
    let probe = serde_json::to_vec(cp).unwrap();
    Ethernet {
        src: &[0x12,0x34,0x56,0x12,0x34,0x56],
        dst: MAC::broadcast().as_ref(),
        ether_type: 0x861,
        payload: probe.as_ref()
    }.write_to_bytes()
}