use std::collections::{HashMap, HashSet};
use crate::representation::Device;
use crate::util::flow::{Flow, FlowOwned};

pub struct CommonState {
    devices: HashMap<String, Device>,
    flows: HashMap<String, HashSet<FlowOwned>>
}

pub trait CommonOperation {
    fn merge_device(&mut self, info:Device) -> MergeResult;
}

impl CommonOperation for CommonState {
    fn merge_device(&mut self, info: Device) -> MergeResult {
        unimplemented!()
    }
}

pub enum MergeResult {
    ADDED,
    MERGED,
    CONFLICT
}