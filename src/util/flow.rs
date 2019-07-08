use crate::util::value::{Value, ParamValue, InnerValue};
use crate::p4rt::helper::P4InfoHelper;
use crate::proto::p4runtime::TableEntry;

pub struct Flow {
    pub device: String,
    pub table: FlowTable,
    pub action: FlowAction,
    pub priority: i32,
    pub metadata: u64
}

impl Flow {
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
}

#[derive(Clone)]
pub struct FlowTable {
    pub name: &'static str,
    pub matches: &'static [(&'static str, InnerValue)]
}

#[derive(Clone)]
pub struct FlowAction {
    pub name: &'static str,
    pub params: &'static [(&'static str, Vec<u8>)]
}