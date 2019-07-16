use std::io::Read;
use std::path::Path;

use protobuf::{Message, SingularPtrField};

use crate::error::*;
use crate::proto;
use crate::proto::p4info::{Action, Action_Param, MatchField, MatchField_MatchType, P4Info, Table};
use crate::proto::p4runtime::{FieldMatch, TableEntry};
use crate::util::value::{InnerParamValue, InnerValue};
use crate::p4rt::pure::adjust_value;

pub struct P4InfoHelper {
    pub p4info:P4Info,
    pub packetout_egress_id: u32
}

impl P4InfoHelper {
    pub fn new(p4info_file_path:&Path) -> P4InfoHelper {
        let mut file = std::fs::File::open(p4info_file_path).unwrap();
        let mut is = protobuf::CodedInputStream::new(&mut file);
        let p4info = protobuf::parse_from_reader(&mut is).unwrap();
        let packetout_id = Self::get_packout_egress_port_metaid(&p4info).unwrap();
        P4InfoHelper {
            p4info,
            packetout_egress_id:packetout_id
        }
    }

    pub fn build_table_entry(&self, table_name:&str, match_fields:&[(&str, InnerValue)],
                             default_action:bool, action_name:&str, action_params:&[(&str, InnerParamValue)], priority:i32
    ) -> TableEntry {
        let mut table_entry = crate::proto::p4runtime::TableEntry::new();
        table_entry.set_table_id(self.get_table_id(table_name).unwrap());

        table_entry.set_priority(priority);

        for (match_field_name, value) in match_fields {
            let entry = self.get_match_field_pb(table_name, match_field_name, value).unwrap();
            table_entry.field_match.push(entry)
        }

        if default_action {
            table_entry.set_is_default_action(true);
        }

        if !action_name.is_empty() {
            let action = table_entry.mut_action().mut_action();
            action.set_action_id(self.get_actions_id(action_name).unwrap());
            if !action_params.is_empty() {
                for (field_name, value) in action_params {
                    action.params.push(self.get_action_param_pb(action_name, field_name, value));
                }
            }
        }
        return table_entry;
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
        self.p4info.tables.iter().for_each(|t|{
            println!("{}",t.preamble.as_ref().unwrap().name);
        });
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

    pub fn get_action(&self, name:&str) -> Option<&Action> {
        self.p4info.actions.iter()
            .filter(|t|t.preamble.is_some())
            .find(|t|{
                let pre = t.preamble.as_ref().unwrap();
                &pre.name==name || &pre.alias==name
            })
    }

    pub fn get_actions_id(&self, action_name:&str) -> Option<u32> {
        self.get_action(action_name).map(|table| {
            table.preamble.as_ref().unwrap().id
        })
    }

    pub fn get_match_field_pb(&self, table_name:&str, match_field_name:&str, value: &InnerValue) -> Option<FieldMatch> {
        let p4info_match = self.get_match_field_by_name(table_name, match_field_name).unwrap();
        let bitwidth = p4info_match.bitwidth;
        let byte_len = (bitwidth as f32 / 8.0).ceil() as usize;;
        println!("{}", bitwidth);
        let byte_len = byte_len as usize;
        let mut p4runtime_match = crate::proto::p4runtime::FieldMatch::new();
        p4runtime_match.set_field_id(p4info_match.id);
        match (p4info_match.get_match_type(),value) {
            (MatchField_MatchType::EXACT, InnerValue::EXACT(v))=>{
//                assert_eq!(byte_len, v.len());
                let v = adjust_value(v.clone(),byte_len);
                p4runtime_match.mut_exact().value = v;
            }
            (MatchField_MatchType::LPM, InnerValue::LPM(v, l))=>{
                assert_eq!(byte_len, v.len());
                p4runtime_match.mut_lpm().prefix_len = *l;
                p4runtime_match.mut_lpm().value = v.clone();
            }
            (MatchField_MatchType::TERNARY, InnerValue::TERNARY(v, mask))=>{
                assert_eq!(byte_len, v.len());
//                assert_eq!(byte_len, mask.len());
                let mask = adjust_value(mask.clone(),byte_len);
                p4runtime_match.mut_ternary().value = v.clone();
                p4runtime_match.mut_ternary().mask = mask;
            }
            (MatchField_MatchType::RANGE, InnerValue::RANGE(low, high))=>{
                assert_eq!(byte_len, low.len());
                assert_eq!(byte_len, high.len());
                p4runtime_match.mut_range().low = low.clone();
                p4runtime_match.mut_range().high = high.clone();
            }
            _=>{
                panic!("what")
            }
        }
        return Some(p4runtime_match);
    }

    pub fn get_action_param_by_name(&self, action_name:&str, param: &str) -> Option<&Action_Param> {
        self.get_action(action_name).map_or(None,|action|{
            for p in action.params.iter() {
                if &p.name == param {
                    return Some(p);
                }
            }
            None
        })
    }

    pub fn get_packout_egress_port_metaid(p4info:&P4Info) -> Option<u32> {
        p4info.get_controller_packet_metadata().iter().find(|p|{
            let pre = p.preamble.as_ref().unwrap();
            pre.name=="packet_out"
        }).map(|x|{
            x.metadata.iter().find(|meta|{
                meta.name=="egress_port"
            }).map(|x|{
                x.id
            })
        }).flatten()
    }

    pub fn get_action_param_pb(&self, action_name:&str, param_name:&str, value: &InnerParamValue) -> crate::proto::p4runtime::Action_Param {
        let p4info_param = self.get_action_param_by_name(action_name, param_name).unwrap();
        let mut p4runtime_param = crate::proto::p4runtime::Action_Param::new();
        let mut value = value.clone();
        let bytes_len = (p4info_param.bitwidth as f32 / 8.0).ceil() as usize;
        println!("adjust value: action:{}, param:{}, value:{:?}, bitwidth:{}",action_name,param_name,value,p4info_param.bitwidth);
        let value = adjust_value(value,bytes_len);
        p4runtime_param.set_param_id(p4info_param.id);
        p4runtime_param.set_value(value.clone());
        return p4runtime_param;
    }
}