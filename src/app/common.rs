use std::collections::HashMap;
use crate::representation::Device;

pub struct CommonState {
    devices: HashMap<String, Device>
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