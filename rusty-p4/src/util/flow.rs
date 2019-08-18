use crate::p4rt::pipeconf::Pipeconf;
use crate::p4rt::pure::build_table_entry;
use crate::proto::p4runtime::TableEntry;
use crate::representation::DeviceID;
use crate::util::value::{InnerValue, Value};
use bytes::Bytes;
use failure::_core::fmt::Debug;
use smallvec::SmallVec;
use std::fmt::Formatter;
use std::net::IpAddr;
use std::sync::Arc;

#[derive(Debug)]
pub struct NewFlow {
    pub table: Arc<NewFlowTable>,
    pub action: Arc<NewFlowAction>,
    pub priority: i32,
    pub metadata: u64,
}

impl NewFlow {
    pub fn to_table_entry(&self, pipeconf: &Pipeconf, metadata: u64) -> TableEntry {
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

#[derive(Debug)]
pub struct NewFlowTable {
    pub name: &'static str,
    pub matches: Vec<NewFlowMatch>,
}

#[derive(Clone, Debug)]
pub struct NewFlowMatch {
    pub name: &'static str,
    pub value: InnerValue,
}

unsafe impl smallvec::Array for NewFlowMatch {
    type Item = NewFlowMatch;

    fn size() -> usize {
        std::mem::size_of::<NewFlowMatch>()
    }

    fn ptr(&self) -> *const Self::Item {
        self as *const NewFlowMatch
    }

    fn ptr_mut(&mut self) -> *mut Self::Item {
        self as *mut NewFlowMatch
    }
}

impl NewFlowTable {
    // TODO: Const generic.
    pub fn new(name: &'static str, matches: Vec<NewFlowMatch>) -> NewFlowTable {
        NewFlowTable { name, matches }
    }
}

#[derive(Debug)]
pub struct NewFlowAction {
    pub name: &'static str,
    pub params: Vec<NewFlowActionParam>,
}

#[derive(Debug)]
pub struct NewFlowActionParam {
    pub name: &'static str,
    pub value: Bytes,
}

unsafe impl smallvec::Array for NewFlowActionParam {
    type Item = NewFlowActionParam;

    fn size() -> usize {
        std::mem::size_of::<NewFlowActionParam>()
    }

    fn ptr(&self) -> *const Self::Item {
        self as *const NewFlowActionParam
    }

    fn ptr_mut(&mut self) -> *mut Self::Item {
        self as *mut NewFlowActionParam
    }
}

#[derive(Debug, Clone, Hash)]
pub struct Flow<'a> {
    pub device: DeviceID,
    pub table: FlowTable<'a>,
    pub action: FlowAction<'a>,
    pub priority: i32,
}

#[derive(Debug, Clone)]
pub struct FlowOwned {
    pub device: DeviceID,
    pub table: FlowTableOwned,
    pub action: FlowActionOwned,
    pub priority: i32,
    pub metadata: u64,
}

impl<'a> Flow<'a> {
    pub fn to_table_entry(&self, pipeconf: &Pipeconf, metadata: u64) -> TableEntry {
        //        let table_entry = build_table_entry(
        //            pipeconf.get_p4info(),
        //            self.table.name,
        //            self.table.matches,
        //            false,
        //            self.action.name,
        //            self.action.params,
        //            self.priority,
        //            metadata,
        //        );
        //        table_entry
        unimplemented!()
    }

    pub fn into_owned(self, metadata: u64) -> FlowOwned {
        FlowOwned {
            device: self.device,
            table: self.table.into_owned(),
            action: self.action.to_owned(),
            priority: self.priority,
            metadata,
        }
    }
}

#[derive(Clone, Debug, Hash)]
pub struct FlowTable<'a> {
    pub name: &'static str,
    pub matches: &'a [(&'static str, InnerValue)],
}

impl<'a> FlowTable<'a> {
    pub fn into_owned(self) -> FlowTableOwned {
        let mut static_table: Vec<(&'static str, InnerValue)> = vec![];
        for m in self.matches {
            static_table.push((*m).clone());
        }
        FlowTableOwned {
            name: self.name,
            matches: static_table,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FlowTableOwned {
    pub name: &'static str,
    pub matches: Vec<(&'static str, InnerValue)>,
}

#[derive(Clone, Debug, Hash)]
pub struct FlowAction<'a> {
    pub name: &'static str,
    pub params: &'a [(&'static str, Vec<u8>)],
}

impl<'a> FlowAction<'a> {
    pub fn to_owned(&self) -> FlowActionOwned {
        let mut static_action: Vec<(&'static str, Vec<u8>)> = vec![];
        for m in self.params {
            static_action.push((*m).clone());
        }
        FlowActionOwned {
            name: self.name,
            params: static_action,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FlowActionOwned {
    pub name: &'static str,
    pub params: Vec<(&'static str, Vec<u8>)>,
}
