use crate::p4rt::helper::P4InfoHelper;
use crate::proto::p4runtime::TableEntry;
use crate::util::value::{InnerValue, ParamValue, Value};

pub struct Flow<'a> {
    pub device: String,
    pub table: FlowTable<'a>,
    pub action: FlowAction<'a>,
    pub priority: i32,
    pub metadata: u64
}

pub struct FlowOwned {
    pub device: String,
    pub table: FlowTableOwned,
    pub action: FlowActionOwned,
    pub priority: i32,
    pub metadata: u64
}

impl<'a> Flow<'a> {
    pub fn to_table_entry(&self, p4info_helper:&P4InfoHelper) -> TableEntry {
        let table_entry = p4info_helper.build_table_entry(
            self.table.name,
            self.table.matches,
            false,
            self.action.name,
            self.action.params,
            0
        );
        table_entry
    }

    pub fn to_owned(&self) -> FlowOwned {
        FlowOwned {
            device: self.device.clone(),
            table: self.table.to_owned(),
            action: self.action.to_owned(),
            priority: self.priority,
            metadata: self.metadata
        }
    }
}

#[derive(Clone)]
pub struct FlowTable<'a> {
    pub name: &'static str,
    pub matches: &'a [(&'static str, InnerValue)]
}

impl<'a> FlowTable<'a> {
    pub fn to_owned(&self) -> FlowTableOwned {
        let mut static_table:Vec<(&'static str, InnerValue)> = vec![];
        for m in self.matches {
            static_table.push((*m).clone());
        }
        FlowTableOwned {
            name: self.name,
            matches: static_table
        }
    }
}

#[derive(Clone)]
pub struct FlowTableOwned {
    pub name: &'static str,
    pub matches: Vec<(&'static str, InnerValue)>
}

#[derive(Clone)]
pub struct FlowAction<'a> {
    pub name: &'static str,
    pub params: &'a [(&'static str, Vec<u8>)]
}

impl<'a> FlowAction<'a> {
    pub fn to_owned(&self) -> FlowActionOwned {
        let mut static_action:Vec<(&'static str, Vec<u8>)> = vec![];
        for m in self.params {
            static_action.push((*m).clone());
        }
        FlowActionOwned {
            name: self.name,
            params: static_action
        }
    }
}

#[derive(Clone)]
pub struct FlowActionOwned {
    pub name: &'static str,
    pub params: Vec<(&'static str, Vec<u8>)>
}