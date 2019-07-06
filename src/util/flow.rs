use crate::util::value::{Value, ParamValue};

pub struct Flow {
    pub device: String,
    pub table: FlowTable,
    pub action: FlowAction,
    pub priority: i32
}

#[derive(Clone)]
pub struct FlowTable {
    pub name: &'static str,
    pub matches: &'static [(&'static str, Value)]
}

pub struct FlowAction {
    pub name: &'static str,
    pub params: &'static [(&'static str, ParamValue)]
}