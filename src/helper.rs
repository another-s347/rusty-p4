use std::path::Path;
use crate::proto;
use protobuf::Message;
use crate::proto::p4info::{P4Info, Table, MatchField, MatchField_MatchType};
use crate::proto::p4runtime::FieldMatch;

pub struct P4InfoHelper {
    pub p4info:P4Info
}

impl P4InfoHelper {
    pub fn new(p4info_file_path:&Path) -> P4InfoHelper {
        let mut file = std::fs::File::open(p4info_file_path).unwrap();
        let mut is = protobuf::CodedInputStream::new(&mut file);
        let mut p4info = proto::p4info::P4Info::new();
        p4info.merge_from(&mut is);
        P4InfoHelper {
            p4info
        }
    }

    pub fn build_table_entry(&self, table_name:&str, match_fields:Option<(&str, u32)>,
        default_action:bool, action_name:&str, action_params:&str, priority:i32
    ) {
        let mut table_entry = crate::proto::p4runtime::TableEntry::new();
        table_entry.set_table_id(self.get_table_id(table_name).unwrap());

        table_entry.set_priority(priority);

        for (match_field_name, value) in match_fields {
            let entry = self.get_match_field_pb(table_name, match_field_name, Vec::new()).unwrap();
            table_entry.field_match.push(entry)
        }
    }

    pub fn get_table(&self, name:&str) -> Option<&Table> {
        self.p4info.tables.iter()
            .filter(|t|t.preamble.is_some())
            .find(|t|{
                let pre = t.preamble.as_ref().unwrap();
                &pre.name==name || &pre.alias==name
            })
    }

    pub fn get_table_id(&self, name:&str) -> Option<u32> {
        self.get_table(name).map(|table| {
            table.preamble.as_ref().unwrap().id
        })
    }

    pub fn get_match_field_by_name(&self, table_name:&str, name:&str) -> Option<&MatchField> {
        for t in self.p4info.tables.iter().filter(|p|p.preamble.is_some()) {
            let pre = t.preamble.as_ref().unwrap();
            if &pre.name == table_name {
                for mf in t.match_fields.iter() {
                    if &mf.name==name {
                        return Some(mf);
                    }
                }
            }
        }
        None
    }

    pub fn get_match_field_by_id(&self, table_name:&str, id:u32) -> Option<&MatchField> {
        for t in self.p4info.tables.iter().filter(|p|p.preamble.is_some()) {
            let pre = t.preamble.as_ref().unwrap();
            if &pre.name == table_name {
                for mf in t.match_fields.iter() {
                    if mf.id == id {
                        return Some(mf);
                    }
                }
            }
        }
        None
    }

    pub fn get_match_field_pb(&self, table_name:&str, match_field_name:&str, value:Vec<u8>) -> Option<FieldMatch> {
        let p4info_match = self.get_match_field_by_name(table_name, match_field_name).unwrap();
        let bitwidth = p4info_match.bitwidth;
        let byte_len = (bitwidth / 8) as usize;
        assert_eq!(byte_len, value.len());
        let mut p4runtime_match = crate::proto::p4runtime::FieldMatch::new();
        p4runtime_match.set_field_id(p4info_match.id);
        // TODO: decide value type
        match p4info_match.get_match_type() {
            MatchField_MatchType::EXACT=>{
                p4runtime_match.mut_exact().value = value;
            }
            MatchField_MatchType::LPM=>{

            }
            MatchField_MatchType::TERNARY=>{

            }
            MatchField_MatchType::RANGE=>{

            }
            _=>{

            }
        }
        return Some(p4runtime_match);
    }
}
