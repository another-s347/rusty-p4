use rusty_p4_proto::proto::v1::{CounterEntry, Index, Entity};
use crate::representation::DeviceID;
use crate::p4rt::pipeconf::Pipeconf;
use crate::p4rt::pure::get_counter_id;
use crate::entity::{ToEntity, ProtoEntity};

#[derive(Clone, Debug)]
pub struct Counter {
    pub name: &'static str,
    pub index: Option<i64>
}

impl Counter {
    pub fn to_index(&self, device:DeviceID, pipeconf:&Pipeconf) -> Option<CounterIndex> {
        let id = get_counter_id(pipeconf.get_p4info(), self.name)?;
        Some(CounterIndex {
            device,
            id,
            index: self.index
        })
    }
}

impl ToEntity for Counter {
    fn to_proto_entity(&self, pipeconf: &Pipeconf) -> Option<Entity> {
        let id = get_counter_id(pipeconf.get_p4info(), self.name)?;
        Some(ProtoEntity {
            entity: Some(crate::proto::p4runtime::entity::Entity::CounterEntry(CounterEntry {
                counter_id: id,
                index: self.index.map(|x|Index {index:x}),
                data: None
            }))
        })
    }
}

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