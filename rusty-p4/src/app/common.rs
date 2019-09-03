use std::collections::{HashMap, HashSet};

use crate::app::async_app::AsyncAppsBuilder;
use crate::app::graph::DefaultGraph;
use crate::app::sync_app::SyncAppsBuilder;
use crate::app::P4app;
use crate::context::ContextHandle;
use crate::event::{CommonEvents, Event, PacketReceived};
use crate::representation::{ConnectPoint, Device, DeviceID, DeviceType, Host, Interface, Link};
use crate::util::flow::*;
use crate::util::value::MAC;
use failure::_core::cell::RefCell;
use log::{info, warn};
use std::convert::TryInto;
use std::rc::Rc;
use std::sync::Arc;

pub struct CommonState {
    pub devices: HashMap<DeviceID, Device>,
    pub flows: HashMap<u64, HashSet<Flow>>,
    pub hosts: HashSet<Host>,
    pub graph: DefaultGraph,
    pub links: HashSet<Link>,
}

impl CommonState {
    pub fn new() -> CommonState {
        CommonState {
            devices: HashMap::new(),
            flows: HashMap::new(),
            hosts: HashSet::new(),
            graph: DefaultGraph::new(),
            links: HashSet::new(),
        }
    }
}

impl CommonState {
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

    pub fn add_link(&mut self, link: Link, cost: u8) -> MergeResult<()> {
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
            .iter()
            .flat_map(|dev| dev.ports.iter())
            .find(|port| port.number == cp.port)
            .map(|port| port.interface.as_ref())
            .flatten()
    }

    pub fn get_mac_by_cp(&self, cp: &ConnectPoint) -> Option<MAC> {
        self.get_interface_by_cp(cp).map(|i| i.mac).flatten()
    }
}

#[derive(Debug)]
pub enum MergeResult<T> {
    ADDED(T),
    MERGED,
    CONFLICT,
}

impl<E> P4app<E> for CommonState
where
    E: Event,
{
    fn on_event(self: &mut Self, event: E, ctx: &ContextHandle<E>) -> Option<E> {
        if let Some(common) = event.try_to_common() {
            match common {
                CommonEvents::DeviceAdded(device) => {
                    if self.devices.contains_key(&device.id) {
                        None
                    } else {
                        self.devices.insert(device.id, device.clone());
                        Some(event)
                    }
                }
                CommonEvents::DeviceUpdate(device) => {
                    let result = self.merge_device(device);
                    match result {
                        MergeResult::ADDED(name) => Some(event),
                        MergeResult::MERGED => Some(event),
                        MergeResult::CONFLICT => None,
                    }
                }
                CommonEvents::DeviceLost(deviceID) => {
                    if let Some(device) = self.devices.remove(&deviceID) {
                        self.hosts
                            .iter()
                            .filter(|host| host.location.device == *deviceID)
                            .for_each(|host| {
                                ctx.send_event(CommonEvents::HostLost(*host).into_e());
                            });
                        self.links
                            .iter()
                            .filter(|x| x.src.device == *deviceID || x.dst.device == *deviceID)
                            .for_each(|link| {
                                ctx.send_event(CommonEvents::LinkLost(*link).into_e());
                            });
                        self.graph.remove_device(&deviceID);
                        info!(target:"extend","device removed {}", device.name);
                        Some(event)
                    } else {
                        warn!(target:"extend","duplicated device lost event {:?}", deviceID);
                        None
                    }
                }
                CommonEvents::HostDetected(host) => {
                    let result = self.merge_host(host);
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
                    let result = self.add_link(link.clone(), 1);
                    match result {
                        MergeResult::ADDED(()) => Some(event),
                        MergeResult::CONFLICT => None,
                        MergeResult::MERGED => Some(event),
                    }
                }
                CommonEvents::LinkLost(link) => {
                    if self.links.remove(&link) {
                        self.graph.remove_link(&link);
                        info!(target:"extend","link removed {:?}", link);
                        return Some(event);
                    } else {
                        warn!(target:"extend","duplicated link lost event {:?}", link);
                        None
                    }
                }
                CommonEvents::HostLost(host) => {
                    if self.hosts.remove(&host) {
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

fn test<E>()
where
    E: Event,
{
    let mut async_builder: AsyncAppsBuilder<E> = super::async_app::AsyncAppsBuilder::new();
    let commonstate_async_service =
        async_builder.with_sync_service(1, "common state", CommonState::new());
    let mut sync_builder: SyncAppsBuilder<E> = super::sync_app::SyncAppsBuilder::new();
    let commonstate_service = sync_builder.with_service(1, "common state", CommonState::new());
}
