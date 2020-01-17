use std::collections::{HashMap, HashSet};
use async_trait::async_trait;
//use crate::app::async_app::AsyncAppsBuilder;
use crate::app::graph::DefaultGraph;
use crate::app::P4app;
use crate::core::DefaultContext;
use crate::event::{CommonEvents, Event, PacketReceived};
use crate::representation::{ConnectPoint, Device, DeviceID, DeviceType, Host, Interface, Link};
use crate::util::flow::*;
use crate::util::value::MAC;
use log::{info, warn};
use std::cell::RefCell;
use std::convert::TryInto;
use std::rc::Rc;
use std::sync::Arc;
use crate::service::{Service};
use parking_lot::Mutex;
use crate::core::context::Context;

#[derive(Clone)]
pub struct CommonState {
    pub inner:Arc<Mutex<CommonStateInner>>
}

pub struct CommonStateInner {
    pub devices: HashMap<DeviceID, Device>,
    pub flows: HashMap<u64, HashSet<Flow>>,
    pub hosts: HashSet<Host>,
    pub graph: DefaultGraph,
    pub links: HashSet<Link>,
}

impl CommonState {
    pub fn new() -> CommonState {
        CommonState {
            inner: Arc::new(Mutex::new(
                CommonStateInner {
                    devices: HashMap::new(),
                    flows: HashMap::new(),
                    hosts: HashSet::new(),
                    graph: DefaultGraph::new(),
                    links: HashSet::new(),
                }
            ))
        }
    }
}

impl CommonStateInner {
    pub fn merge_device(&mut self, mut info: &Device) -> MergeResult<DeviceID> {
        let id = info.id;
        if let Some(pre) = self.devices.get_mut(&id) {
            // merge ports
            for port in &info.ports {
                pre.ports.insert(port.clone());
            }
            MergeResult::MERGED
        } else {
            // add device
            self.graph.add_device(info);
            self.devices.insert(id, info.clone());
            MergeResult::ADDED(id)
        }
    }

    pub fn merge_host(&mut self, info: &Host) -> MergeResult<Host> {
        if let Some(other) = self.hosts.get(info) {
            if other.location != info.location {
                MergeResult::CONFLICT
            } else {
                MergeResult::MERGED
            }
        } else {
            let result = info.clone();
            self.hosts.insert(info.clone());
            MergeResult::ADDED(result)
        }
    }

    pub fn add_link(&mut self, link: Link, cost: u32) -> MergeResult<()> {
        let result = self.graph.add_link(&link, cost);
        match result {
            MergeResult::CONFLICT => {}
            MergeResult::ADDED(()) => {
                self.links.insert(link);
            }
            MergeResult::MERGED => {}
        }
        result
    }

    pub fn get_interface_by_cp(&self, cp: &ConnectPoint) -> Option<&Interface> {
        self.devices
            .get(&cp.device)
            .and_then(|dev|{
                dev.ports.iter()
                    .find(|port| port.number == cp.port)
                    .and_then(|port| port.interface.as_ref())
            })
    }

    pub fn get_mac_by_cp(&self, cp: &ConnectPoint) -> Option<MAC> {
        self.get_interface_by_cp(cp).and_then(|i| i.mac)
    }
}

#[derive(Debug)]
pub enum MergeResult<T> {
    ADDED(T),
    MERGED,
    CONFLICT,
}

#[async_trait]
impl<E, C> P4app<E, C> for CommonState
where
    E: Event,
    C: Context<E>
{
    async fn on_event(self: &mut Self, event: E, ctx: &mut C) -> Option<E> {
        if let Some(common) = event.try_to_common() {
            let mut inner = self.inner.lock();
            match common {
                CommonEvents::DeviceAdded(device) => {
                    if inner.devices.contains_key(&device.id) {
                        None
                    } else {
                        inner.graph.add_device(&device);
                        inner.devices.insert(device.id, device.clone());
                        Some(event)
                    }
                }
                CommonEvents::DeviceUpdate(device) => {
                    let result = inner.merge_device(device);
                    match result {
                        MergeResult::ADDED(name) => Some(event),
                        MergeResult::MERGED => Some(event),
                        MergeResult::CONFLICT => None,
                    }
                }
                CommonEvents::DeviceLost(deviceID) => {
                    if let Some(device) = inner.devices.remove(&deviceID) {
                        inner.hosts
                            .iter()
                            .filter(|host| host.location.device == *deviceID)
                            .for_each(|host| {
                                ctx.send_event(CommonEvents::HostLost(*host).into_e());
                            });
                        inner.links
                            .iter()
                            .filter(|x| x.src.device == *deviceID || x.dst.device == *deviceID)
                            .for_each(|link| {
                                ctx.send_event(CommonEvents::LinkLost(*link).into_e());
                            });
                        inner.graph.remove_device(&deviceID);
                        info!(target:"extend","device removed {}", device.name);
                        Some(event)
                    } else {
                        warn!(target:"extend","duplicated device lost event {:?}", deviceID);
                        None
                    }
                }
                CommonEvents::HostDetected(host) => {
                    let result = inner.merge_host(host);
                    match result {
                        MergeResult::ADDED(host) => {
                            info!(target:"extend","host detected {:?}",host);
                            Some(event)
                        }
                        MergeResult::CONFLICT => None,
                        MergeResult::MERGED => Some(event),
                    }
                }
                CommonEvents::LinkDetected(link) => {
                    let result = inner.add_link(link.clone(), 1);
                    match result {
                        MergeResult::ADDED(()) => Some(event),
                        MergeResult::CONFLICT => None,
                        MergeResult::MERGED => Some(event),
                    }
                }
                CommonEvents::LinkLost(link) => {
                    if inner.links.remove(&link) {
                        inner.graph.remove_link(&link);
                        info!(target:"extend","link removed {:?}", link);
                        return Some(event);
                    } else {
                        warn!(target:"extend","duplicated link lost event {:?}", link);
                        None
                    }
                }
                CommonEvents::HostLost(host) => {
                    if inner.hosts.remove(&host) {
                        info!(target:"extend","host removed {:?}", &host);
                        Some(event)
                    } else {
                        warn!(target:"extend","duplicated host lost event {:?}", &host);
                        None
                    }
                }
                _ => Some(event),
            }
        } else {
            Some(event)
        }
    }
}

impl Service for CommonState {
    type ServiceType = CommonState;

    fn get_service(&mut self) -> Self::ServiceType {
        CommonState {
            inner: self.inner.clone()
        }
    }
}