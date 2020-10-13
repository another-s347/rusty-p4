use bytes::Bytes;
use futures::prelude::*;
use log::{debug, error, info, trace, warn};
use rusty_p4::representation::{ConnectPoint, Device, DeviceID, Link};
use rusty_p4::util::flow::*;
use rusty_p4::util::packet::Ethernet;
use rusty_p4::util::packet::Packet;
use rusty_p4::util::value::{Value, EXACT, MAC};
use rusty_p4::{
    event::{CommonEvents, CoreRequest, Event, PacketReceived},
    util::publisher::Handler,
};
use std::time::Duration;
//use rusty_p4::app::extended::{P4appInstallable, P4appExtendedCore, EtherPacketHook};
use rusty_p4::app::common::CommonState;
use rusty_p4::representation::DeviceType;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
//use futures::prelude::*;
use async_trait::async_trait;
use rusty_p4::app::App;
use rusty_p4::p4rt::pipeconf::{DefaultPipeconf, PipeconfID};
use rusty_p4::util::flow::Flow;
use std::any::Any;
use tokio::sync::oneshot::Sender;
// use rusty_p4::core::context::Context;

pub struct LinkProbeLoader {
    interceptor: HashMap<PipeconfID, Box<dyn LinkProbeInterceptor>>,
}

#[derive(Clone)]
pub struct LinkProbeState {
    pub inner: Arc<Mutex<HashMap<DeviceID, Vec<Sender<()>>>>>,
    pub interceptor: Arc<HashMap<PipeconfID, Box<dyn LinkProbeInterceptor>>>,
    pub bmv2_manager: rusty_p4::p4rt::bmv2::Bmv2Manager,
    pub device_manager: rusty_p4::app::device_manager::DeviceManager
}

pub trait LinkProbeInterceptor: Sync + Send {
    fn new_flow(&self, device: DeviceID) -> Flow;
}

// impl LinkProbeLoader {
//     pub fn new() -> Self {
//         LinkProbeLoader {
//             interceptor: HashMap::new(),
//         }
//     }

//     pub fn with_interceptor<T: 'static>(mut self, pipeconf: &str, interceptor: T) -> Self
//     where
//         T: LinkProbeInterceptor,
//     {
//         let pipeconf = rusty_p4::util::hash(pipeconf);
//         self.interceptor
//             .insert(PipeconfID(pipeconf), Box::new(interceptor));
//         self
//     }

//     pub fn build(self) -> LinkProbeState {
//         LinkProbeState {
//             inner: Arc::new(Mutex::new(Default::default())),
//             interceptor: Arc::new(self.interceptor),
//         }
//     }
// }

#[async_trait]
impl App for LinkProbeState {
    type Dependency = tuple_list::tuple_list_type!(rusty_p4::p4rt::bmv2::Bmv2Manager, rusty_p4::app::device_manager::DeviceManager);

    type Option = ();

    fn init<S>(dependencies: Self::Dependency, store: &mut S, option: Self::Option) -> Self
    where
        S: rusty_p4::app::store::AppStore,
    {
        let tuple_list::tuple_list!(manager, device_manager) = dependencies;

        let app = Self {
            inner: todo!(),
            interceptor: todo!(),
            bmv2_manager: manager.clone(),
            device_manager: device_manager.clone()
        };

        manager.subscribe_packet(app);
        device_manager.subscribe(app);

        todo!()
    }

    async fn run(&self) {
        todo!()
    }

    // async fn on_packet(&mut self, packet: PacketReceived, ctx: &mut C) -> Option<PacketReceived> {
    //     match Ethernet::<&[u8]>::from_bytes(&packet.packet) {
    //         Some(ref ethernet) if ethernet.ether_type==0x861 => {
    //             let probe:Result<ConnectPoint,serde_json::Error> = serde_json::from_slice(&ethernet.payload);
    //             if let Ok(from) = probe {
    //                 let this = ctx.get_connectpoint(&packet).unwrap();
    //                 let from = from.to_owned();
    //                 ctx.send_event(CommonEvents::LinkDetected(Link {
    //                     src: from,
    //                     dst: this
    //                 }).into_e());
    //                 return None;
    //             }
    //             else {
    //                 error!(target:"linkprobe","invalid probe == {:?}",probe);
    //             }
    //         }
    //         _ => {}
    //     }
    //     Some(packet)
    // }

    // async fn on_event(&mut self, event: E, ctx: &mut C) -> Option<E> {
    //     match event.try_to_common() {
    //         Some(CommonEvents::DeviceAdded(device)) => {
    //             on_device_added(self,device,ctx);
    //         }
    //         Some(CommonEvents::DeviceLost(device)) => {
    //             let mut s = self.inner.lock().unwrap();
    //             if let Some(list) = s.remove(&device) {
    //                 info!(target:"extend","cancel link probe task for device: {:?}",device);
    //                 for x in list {
    //                     x.send(());
    //                 }
    //             }
    //         }unimplemented!();
    //         _=>{}
    //     }
    //     Some(event)
    // }
}

#[async_trait]
impl Handler<rusty_p4::event::PacketReceived> for LinkProbeState {
    async fn handle(&self, packet: rusty_p4::event::PacketReceived) {
        match Ethernet::<&[u8]>::from_bytes(&packet.packet) {
            Some(ref ethernet) if ethernet.ether_type == 0x861 => {
                let probe: Result<ConnectPoint, serde_json::Error> =
                    serde_json::from_slice(&ethernet.payload);
                if let Ok(from) = probe {
                    let this = self.bmv2_manager.get_packet_connectpoint(&packet).unwrap();
                    let from = from.to_owned();
                    // ctx.send_event(
                    //     CommonEvents::LinkDetected(Link {
                    //         src: from,
                    //         dst: this,
                    //     })
                    //     .into_e(),
                    // );
                } else {
                    error!(target:"linkprobe","invalid probe == {:?}",probe);
                }
            }
            _ => {}
        }
    }
}

#[async_trait]
impl Handler<rusty_p4::app::device_manager::DeviceEvent> for LinkProbeState {
    async fn handle(&self, event: rusty_p4::app::device_manager::DeviceEvent) {
        match event {
            rusty_p4::app::device_manager::DeviceEvent::DeviceAdded(device) => {
                let mut bmv2_device = self.bmv2_manager.get_device(device).unwrap();
                let device = self.device_manager.get_device(device);
                let flow = bmv2_device.pipeconf.as_ref()
                    .and_then(|x|x.get_behaviour::<Box<dyn LinkProbeInterceptor>>("a"))
                    .map(|x|x.new_flow(device.id))
                    .unwrap();
                bmv2_device.insert_flow(flow).await.unwrap();
                let mut linkprobe_per_ports = Vec::new();
                for port in device.ports.iter().map(|x| x.number) {
                    let cp = ConnectPoint {
                        device: device.id,
                        port,
                    };
                    let probe = new_probe(&cp);
                    let mut interval = tokio::time::interval(Duration::new(3, 0));
                    let (cancel, mut cancel_r) = tokio::sync::oneshot::channel();
                    let mut handle = bmv2_device.get_handle();
                    tokio::spawn(async move {
                        while let Some(s) = interval.next().await {
                            if cancel_r.try_recv().is_ok() {
                                break;
                            }
                            handle.packet_out(cp.port, probe.clone()).await.unwrap();
                        }
                    });
                    linkprobe_per_ports.push(cancel);
                }
                if !linkprobe_per_ports.is_empty() {
                    info!(target:"linkprobe","start probe for device: {:?}",device.id);
                }
                let mut tasks = self.inner.lock().unwrap();
                tasks.insert(device.id, linkprobe_per_ports);
            }
        }
    }
}

pub fn new_probe(cp: &ConnectPoint) -> Bytes {
    let probe = serde_json::to_vec(cp).unwrap();
    Ethernet {
        src: &[0x12, 0x34, 0x56, 0x12, 0x34, 0x56],
        dst: MAC::broadcast().as_ref(),
        ether_type: 0x861,
        payload: probe.as_ref(),
    }
    .write_to_bytes()
}
