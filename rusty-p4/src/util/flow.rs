use crate::p4rt::pipeconf::Pipeconf;
use crate::p4rt::pure::build_table_entry;
use crate::proto::p4runtime::TableEntry;
use crate::representation::DeviceID;
use crate::util::value::{InnerValue, Value};
use bytes::Bytes;
use failure::_core::fmt::Debug;
//use smallvec::SmallVec;
use std::fmt::Formatter;
use std::net::IpAddr;
use std::sync::Arc;

#[derive(Debug, Hash)]
pub struct Flow {
    pub table: Arc<FlowTable>,
    pub action: Arc<FlowAction>,
    pub priority: i32,
    pub metadata: u64,
}

impl Flow {
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

#[derive(Debug, Hash)]
pub struct FlowTable {
    pub name: &'static str,
    pub matches: Vec<FlowMatch>,
}

#[derive(Clone, Debug, Hash)]
pub struct FlowMatch {
    pub name: &'static str,
    pub value: InnerValue,
}

//unsafe impl smallvec::Array for FlowMatch {
//    type Item = FlowMatch;
//
//    fn size() -> usize {
//        std::mem::size_of::<FlowMatch>()
//    }
//
//    fn ptr(&self) -> *const Self::Item {
//        self as *const FlowMatch
//    }
//
//    fn ptr_mut(&mut self) -> *mut Self::Item {
//        self as *mut FlowMatch
//    }
//}

impl FlowTable {
    // TODO: Const generic.
    pub fn new(name: &'static str, matches: Vec<FlowMatch>) -> FlowTable {
        FlowTable { name, matches }
    }
}

#[derive(Debug, Hash)]
pub struct FlowAction {
    pub name: &'static str,
    pub params: Vec<FlowActionParam>,
}

#[derive(Debug, Hash)]
pub struct FlowActionParam {
    pub name: &'static str,
    pub value: Bytes,
}

//unsafe impl smallvec::Array for FlowActionParam {
//    type Item = FlowActionParam;
//
//    fn size() -> usize {
//        std::mem::size_of::<FlowActionParam>()
//    }
//
//    fn ptr(&self) -> *const Self::Item {
//        self as *const FlowActionParam
//    }
//
//    fn ptr_mut(&mut self) -> *mut Self::Item {
//        self as *mut FlowActionParam
//    }
//}
