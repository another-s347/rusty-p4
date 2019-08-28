use std::collections::{HashMap, HashSet};

use crate::app::graph::DefaultGraph;
use crate::context::ContextHandle;
use crate::event::Event;
use crate::representation::{ConnectPoint, Device, DeviceID, DeviceType, Host, Interface, Link};
use crate::util::flow::*;
use crate::util::value::MAC;

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
    pub fn merge_device(&mut self, mut info: Device) -> MergeResult<DeviceID> {
        let id = info.id;
        if let Some(pre) = self.devices.get_mut(&id) {
            // merge ports
            for port in info.ports {
                pre.ports.insert(port);
            }
            MergeResult::MERGED
        } else {
            // add device
            self.graph.add_device(&info);
            self.devices.insert(id, info);
            MergeResult::ADDED(id)
        }
    }

    pub fn merge_host(&mut self, info: Host) -> MergeResult<Host> {
        if let Some(other) = self.hosts.get(&info) {
            if other.location != info.location {
                MergeResult::CONFLICT
            } else {
                MergeResult::MERGED
            }
        } else {
            let result = info.clone();
            self.hosts.insert(info);
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
