use crate::p4rt::pipeconf::{DefaultPipeconf, Pipeconf};
use crate::p4rt::pure::build_table_entry;
use crate::proto::p4runtime::TableEntry;
use crate::representation::DeviceID;
use crate::util::value::{InnerValue, Value};
use bytes::Bytes;
use smallvec::SmallVec;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::net::IpAddr;
use std::sync::Arc;

#[derive(Debug, Hash, Clone)]
pub struct Flow {
    pub table: FlowTable,
    pub action: FlowAction,
    pub priority: i32,
    pub metadata: u64,
}

impl Flow {
    pub fn to_table_entry<T>(&self, pipeconf: &T, metadata: u64) -> TableEntry
    where
        T: Pipeconf,
    {
        let table_entry = build_table_entry(
            pipeconf.get_p4info(),
            self.table.name,
            self.table.matches.as_ref(),
            false,
            self.action.name,
            self.action.params.as_ref(),
            self.priority,
            metadata,
        );
        table_entry
    }
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct FlowTable {
    pub name: &'static str,
    pub matches: Arc<SmallVec<[FlowMatch; 3]>>,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct FlowMatch {
    pub name: &'static str,
    pub value: InnerValue,
}

impl FlowTable {
    pub fn new(name: &'static str, matches: Arc<SmallVec<[FlowMatch; 3]>>) -> FlowTable {
        FlowTable { name, matches }
    }

    pub fn merge_matches(&mut self, other: &SmallVec<[FlowMatch; 3]>) {
        // Since our flow matches are usually small, so take the easy way to merge.
        let ours = Arc::make_mut(&mut self.matches);
        merge_matches(ours, other);
    }
}

#[derive(Debug, Hash, Clone)]
pub struct FlowAction {
    pub name: &'static str,
    pub params: Arc<SmallVec<[FlowActionParam; 3]>>,
}

#[derive(Debug, Hash)]
pub struct FlowActionParam {
    pub name: &'static str,
    pub value: Bytes,
}

pub fn merge_matches(ours: &mut SmallVec<[FlowMatch; 3]>, other: &SmallVec<[FlowMatch; 3]>) {
    let len = ours.len();
    for i in other.iter() {
        if ours[0..len].iter().find(|x| x.name == i.name).is_none() {
            ours.push(i.clone());
        }
    }
    ours.sort_by(|a, b| a.name.cmp(b.name));
}
