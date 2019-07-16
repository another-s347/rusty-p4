use std::collections::{HashMap, HashSet};

use crate::representation::{Device, DeviceType, Host};
use crate::util::flow::{Flow, FlowOwned};
use crate::context::ContextHandle;
use crate::event::Event;

pub struct CommonState {
    pub devices: HashMap<String, Device>,
    pub flows: HashMap<String, HashSet<FlowOwned>>,
    pub hosts: HashSet<Host>
}

impl CommonState {
    pub fn new() -> CommonState {
        CommonState {
            devices: HashMap::new(),
            flows: HashMap::new(),
            hosts: HashSet::new()
        }
    }
}

impl CommonState {
    pub fn merge_device(&mut self, mut info: Device) -> MergeResult<String>
    {
        let name = info.name.clone();
        if let Some(pre) = self.devices.get_mut(&name) {
            // merge ports
            unimplemented!()
        }
        else { // add device
            self.devices.insert(info.name.clone(), info);
        }
        MergeResult::ADDED(name)
    }

    pub fn merge_host(&mut self, info:Host) -> MergeResult<Host> {
        if let Some(other) = self.hosts.get(&info) {
            if other.location!=info.location {
                MergeResult::CONFLICT
            }
            else {
                MergeResult::MERGED
            }
        }
        else {
            let result = info.clone();
            self.hosts.insert(info);
            MergeResult::ADDED(result)
        }
    }
}

pub enum MergeResult<T> {
    ADDED(T),
    MERGED,
    CONFLICT
}