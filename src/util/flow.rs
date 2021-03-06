use crate::p4rt::pipeconf::{Pipeconf, DefaultPipeconf};
use crate::p4rt::pure::build_table_entry;
use crate::proto::p4runtime::TableEntry;
use crate::representation::DeviceID;
use crate::util::value::{InnerValue, Value};
use bytes::Bytes;
use std::fmt::Debug;
//use smallvec::SmallVec;
use std::fmt::Formatter;
use std::net::IpAddr;
use std::sync::Arc;

#[derive(Debug, Hash, Clone)]
pub struct Flow {
    pub table: Arc<FlowTable>,
    pub action: Arc<FlowAction>,
    pub priority: i32,
    pub metadata: u64,
}

impl Flow {
    pub fn to_table_entry<T>(&self, pipeconf: &T, metadata: u64) -> TableEntry 
    where T: Pipeconf
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
    pub matches: Vec<FlowMatch>,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
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
