use crate::util::value::{Value, ParamValue, InnerValue};
use crate::p4rt::helper::P4InfoHelper;
use crate::proto::p4runtime::TableEntry;

pub struct Flow<'a> {
    pub device: String,
    pub table: FlowTable<'a>,
    pub action: FlowAction<'a>,
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
}

#[derive(Clone)]
pub struct FlowTable<'a> {
    pub name: &'static str,
    pub matches: &'a [(&'static str, InnerValue)]
}

#[derive(Clone)]
pub struct FlowAction<'a> {
    pub name: &'static str,
    pub params: &'a [(&'static str, Vec<u8>)]
}