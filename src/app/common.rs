use std::collections::{HashMap, HashSet};

use crate::representation::{Device, DeviceType};
use crate::util::flow::{Flow, FlowOwned};
use crate::context::ContextHandle;
use crate::event::Event;

pub struct CommonState {
    devices: HashMap<String, Device>,
    flows: HashMap<String, HashSet<FlowOwned>>
}

impl CommonState {
    pub fn new() -> CommonState {
        CommonState {
            devices: HashMap::new(),
            flows: HashMap::new()
        }
    }
}

pub trait CommonOperation<E> where E:Event {
    fn merge_device(&mut self, info:Device, ctx:&ContextHandle<E>) -> MergeResult;
}

impl<E> CommonOperation<E> for CommonState where E:Event {
    fn merge_device(&mut self, mut info: Device, ctx:&ContextHandle<E>) -> MergeResult
    {
        unimplemented!()
//        if let Some(pre) = self.devices.get_mut(&info.name) {
//            // merge ports
//            unimplemented!()
//        }
//        else { // add device
//            match &info.typ {
//                DeviceType::MASTER {
//                    management_address
//                } => {
//                    ctx.add_device(info.name.clone(), management_address.clone(), info.device_id);
//                    info.index = self.devices.len();
//                    self.devices.insert(info.name.clone(), info);
//                },
//                DeviceType::VIRTUAL => {
//                    self.devices.insert(info.name.clone(), info);
//                },
//            }
//        }
//        MergeResult::ADDED
    }
}

pub enum MergeResult {
    ADDED,
    MERGED,
    CONFLICT
}