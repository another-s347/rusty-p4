use rusty_p4_proto::proto::v1::{CounterEntry, Index};
use crate::representation::DeviceID;

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct CounterIndex {
    pub device: DeviceID,
    pub id:u32,
    pub index:Option<i64>
}

impl CounterIndex {
    pub fn to_counter_entry(&self) -> CounterEntry {
        CounterEntry {
            counter_id: self.id,
            index: self.index.map(|i|Index {
                index: i
            }),
            data: None
        }
    }

    pub fn from_counter_entry(entry:&CounterEntry, device:DeviceID) -> CounterIndex {
        CounterIndex {
            device,
            id: entry.counter_id,
            index: entry.index.as_ref().map(|i|i.index)
        }
    }
}