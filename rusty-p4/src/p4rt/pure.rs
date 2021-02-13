use log::{debug, error, info, trace, warn};

use crate::entity::UpdateType;
use crate::error::{ConnectionError, ConnectionErrorKind};
use crate::p4rt::pipeconf::DefaultPipeconf;
use crate::proto::p4config::P4Info;
use crate::proto::p4config::*;
use crate::proto::p4runtime::{
    field_match, stream_message_request, FieldMatch, StreamMessageRequest, TableEntry, WriteRequest,
};
use crate::util::flow::{FlowActionParam, FlowMatch, Flow, FlowTable, FlowAction};
use crate::util::value::{Encode, InnerParamValue, InnerValue};
use byteorder::BigEndian;
use byteorder::ByteOrder;
use bytes::{Bytes, BytesMut};
use failure::{ResultExt, Fail};
use futures::{Future, Sink, StreamExt};
use nom::{dbg_dmp, ExtendInto};
use rusty_p4_proto::proto::v1::{
    Entity, Index, MeterConfig, MeterEntry, PacketMetadata, PacketOut, TableAction, Uint128, Update,MasterArbitrationUpdate,
};
use std::path::Path;
use tokio::io::AsyncReadExt;
use crate::p4rt::bmv2::Bmv2MasterUpdateOption;
use std::sync::Arc;
use rusty_p4_proto::proto::v1::field_match::{FieldMatchType, Exact, Ternary, Lpm, Range};
use super::pipeconf::Pipeconf;

pub fn new_write_table_entry(
    device_id: u64,
    table_entry: TableEntry,
    update: UpdateType,
) -> WriteRequest {
    let update_type = if table_entry.is_default_action {
        crate::proto::p4runtime::update::Type::Modify
    } else {
        update.into()
    };
    let mut request = crate::proto::p4runtime::WriteRequest {
        device_id,
        role_id: 0,
        election_id: Some(Uint128 { high: 0, low: 1 }),
        updates: vec![Update {
            r#type: update_type as i32,
            entity: Some(Entity {
                entity: Some(crate::proto::p4runtime::entity::Entity::TableEntry(
                    table_entry.clone(),
                )),
            }),
        }],
        atomicity: 0,
    };
    request
}

pub fn adjust_value(mut value: Bytes, bytes_len: usize) -> Bytes {
    if bytes_len == value.len() {
        value
    } else if bytes_len < value.len() {
        value.slice(value.len() - bytes_len..value.len())
    } else {
        let mut value2 = BytesMut::from(value.as_ref());
        value2.extend(vec![0u8; bytes_len - value.len()]);
        value2.freeze()
    }
}

pub fn adjust_value_with(value: Bytes, bytes_len: usize, e: u8) -> Bytes {
    if bytes_len == value.len() {
        value
    } else if bytes_len < value.len() {
        value.slice(value.len() - bytes_len..value.len())
    } else {
        let mut value2 = BytesMut::from(value.as_ref());
        value2.extend(vec![e; bytes_len - value.len()]);
        value2.freeze()
    }
}

pub fn new_packet_out_request<T>(
    pipeconf: &T,
    egress_port: u32,
    packet: Bytes,
) -> StreamMessageRequest 
where T: Pipeconf
{
    let packetOut = PacketOut {
        payload: packet,
        metadata: vec![PacketMetadata {
            metadata_id: pipeconf.get_packetout_egress_id(),
            value: adjust_value(Bytes::copy_from_slice(egress_port.to_be_bytes().as_ref()), 2),
        }],
    };
    let request = StreamMessageRequest {
        update: Some(stream_message_request::Update::Packet(packetOut)),
    };
    request
}

pub fn new_set_entity_request(
    device_id: u64,
    entity: Entity,
    update_type: crate::proto::p4runtime::update::Type,
) -> WriteRequest {
    WriteRequest {
        device_id,
        role_id: 0,
        election_id: Some(Uint128 { high: 0, low: 1 }),
        updates: vec![Update {
            r#type: update_type as i32,
            entity: Some(entity),
        }],
        atomicity: 0,
    }
}

pub fn build_table_entry(
    p4info: &P4Info,
    table_name: &str,
    match_fields: &[FlowMatch],
    default_action: bool,
    action_name: &str,
    action_params: &[FlowActionParam],
    priority: i32,
    metadata: u64,
) -> TableEntry {
    let action = if !action_name.is_empty() {
        let action_id = get_actions_id(p4info, action_name);
        let action_id = if action_id.is_none() {
            panic!("action with name '{}' not found.", action_name);
        } else {
            action_id.unwrap()
        };
        let mut action = crate::proto::p4runtime::Action {
            action_id: get_actions_id(p4info, action_name).unwrap(),
            params: vec![],
        };
        if !action_params.is_empty() {
            for p in action_params {
                action.params.push(get_action_param_pb(
                    p4info,
                    action_name,
                    p.name,
                    p.value.clone(),
                ));
            }
        }
        Some(action)
    } else {
        None
    };

    let mut table_entry = TableEntry {
        metadata: Bytes::new(),
        table_id: get_table_id(p4info, table_name).unwrap(),
        r#match: vec![],
        action: Some(TableAction {
            r#type: action.map(crate::proto::p4runtime::table_action::Type::Action),
        }),
        priority,
        controller_metadata: metadata,
        meter_config: None,
        counter_data: None,
        is_default_action: default_action,
        idle_timeout_ns: 0,
        time_since_last_hit: None,
    };

    for m in match_fields {
        let entry = get_match_field_pb(p4info, table_name, m.name, &m.value).unwrap();
        table_entry.r#match.push(entry)
    }

    table_entry
}

//pub fn table_entry_to_flow(
//    p4info: &P4Info,
//    table_entry: &TableEntry
//) -> Option<crate::util::flow::Flow> {
//    let table_id = table_entry.table_id;
//    let table:&Table = p4info
//        .tables
//        .iter()
//        .filter(|x|x.preamble.is_some())
//        .find(|x|x.preamble.as_ref().unwrap().id == table_id)?;
//    let table_name = &table.preamble.as_ref().unwrap().name;
//    let matches = &table.match_fields;
//
//    let mut flow_matches = vec![];
//    for i in table_entry.r#match {
//        if i.field_match_type.is_none() {
//            continue;
//        }
//        let m:&MatchField = matches
//            .iter()
//            .find(|x|x.id == i.field_id)?;
//        let match_name = &m.name;
//        let match_value = match i.field_match_type.unwrap() {
//            FieldMatchType::Exact(Exact { value }) => { InnerValue::EXACT(Bytes::from(value)) }
//            FieldMatchType::Ternary(Ternary { value, mask }) => { InnerValue::TERNARY(Bytes::from(value),Bytes::from(mask)) }
//            FieldMatchType::Lpm(Lpm { value, prefix_len }) => { InnerValue::LPM(Bytes::from(value), prefix_len) }
//            FieldMatchType::Range(Range { low, high }) => { InnerValue::RANGE(Bytes::from(low), Bytes::from(high)) }
//            FieldMatchType::Other(_) => {
//                continue;
//            }
//        };
//        flow_matches.push(FlowMatch {
//            name: "",
//            value: match_value
//        });
//    }
//
//    Flow {
//        table: Arc::new(FlowTable {
//            name: table_name,
//            matches: flow_matches
//        }),
//        action: Arc::new(FlowAction { name: "", params: vec![] }),
//        priority: 0,
//        metadata: 0
//    };
//    unimplemented!()
//}

pub fn table_entry_to_entity(table_entry: TableEntry) -> Entity {
    Entity {
        entity: Some(crate::proto::p4runtime::entity::Entity::TableEntry(
            table_entry,
        )),
    }
}

pub fn get_table<'a>(pipeconf: &'a P4Info, name: &str) -> Option<&'a Table> {
    pipeconf
        .tables
        .iter()
        .filter(|t| t.preamble.is_some())
        .find(|t| {
            let pre = t.preamble.as_ref().unwrap();
            &pre.name == name || &pre.alias == name
        })
}

pub fn get_table_id(pipeconf: &P4Info, name: &str) -> Option<u32> {
    //        self.p4info.tables.iter().for_each(|t|{
    //            println!("{}",t.preamble.as_ref().unwrap().name);
    //        });
    get_table(pipeconf, name).map(|table| table.preamble.as_ref().unwrap().id)
}

pub fn get_match_field_by_name<'a>(
    pipeconf: &'a P4Info,
    table_name: &str,
    name: &str,
) -> Option<&'a MatchField> {
    for t in pipeconf.tables.iter().filter(|p| p.preamble.is_some()) {
        let pre = t.preamble.as_ref().unwrap();
        if &pre.name == table_name {
            for mf in t.match_fields.iter() {
                if &mf.name == name {
                    return Some(mf);
                }
            }
        }
    }
    None
}

pub fn get_match_field_by_id<'a>(
    pipeconf: &'a P4Info,
    table_name: &str,
    id: u32,
) -> Option<&'a MatchField> {
    for t in pipeconf.tables.iter().filter(|p| p.preamble.is_some()) {
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

pub fn get_action<'a>(pipeconf: &'a P4Info, name: &str) -> Option<&'a Action> {
    pipeconf
        .actions
        .iter()
        .filter(|t| t.preamble.is_some())
        .find(|t| {
            let pre = t.preamble.as_ref().unwrap();
            &pre.name == name || &pre.alias == name
        })
}

pub fn get_meter<'a>(pipeconf: &'a P4Info, name: &str) -> Option<&'a Meter> {
    pipeconf
        .meters
        .iter()
        .filter(|t| t.preamble.is_some())
        .find(|t| {
            let pre = t.preamble.as_ref().unwrap();
            &pre.name == name || &pre.alias == name
        })
}

pub fn get_meter_id(pipeconf: &P4Info, name: &str) -> Option<u32> {
    get_meter(pipeconf, name).map(|table| table.preamble.as_ref().unwrap().id)
}

pub fn get_counter<'a>(pipeconf: &'a P4Info, name: &str) -> Option<&'a Counter> {
    pipeconf
        .counters
        .iter()
        .filter(|t| t.preamble.is_some())
        .find(|t| {
            let pre = t.preamble.as_ref().unwrap();
            &pre.name == name || &pre.alias == name
        })
}

pub fn get_counter_id(pipeconf: &P4Info, name:&str) -> Option<u32> {
    get_counter(pipeconf, name).map(|table| table.preamble.as_ref().unwrap().id)
}

pub fn get_directcounter<'a>(pipeconf: &'a P4Info, name: &str) -> Option<&'a DirectCounter> {
    pipeconf
        .direct_counters
        .iter()
        .filter(|t| t.preamble.is_some())
        .find(|t| {
            let pre = t.preamble.as_ref().unwrap();
            &pre.name == name || &pre.alias == name
        })
}

pub fn get_directcounter_id(pipeconf: &P4Info, name:&str) -> Option<u32> {
    get_directcounter(pipeconf, name).map(|table| table.preamble.as_ref().unwrap().id)
}

pub fn get_actions_id(pipeconf: &P4Info, action_name: &str) -> Option<u32> {
    get_action(pipeconf, action_name).map(|table| table.preamble.as_ref().unwrap().id)
}

pub fn get_match_field_pb(
    pipeconf: &P4Info,
    table_name: &str,
    match_field_name: &str,
    value: &InnerValue,
) -> Option<FieldMatch> {
    let p4info_match = get_match_field_by_name(pipeconf, table_name, match_field_name).unwrap();
    let bitwidth = p4info_match.bitwidth;
    let byte_len = (bitwidth as f32 / 8.0).ceil() as usize;
    let x = p4info_match.r#match.as_ref().map(|x| {
        match x {
            match_field::Match::MatchType(x) => {
                match (match_field::MatchType::from_i32(*x), value) {
                    (Some(match_field::MatchType::Exact), InnerValue::EXACT(v)) => {
                        //                assert_eq!(byte_len, v.len());
                        let v = adjust_value(v.clone(), byte_len);
                        field_match::FieldMatchType::Exact(
                            crate::proto::p4runtime::field_match::Exact { value: v },
                        )
                    }
                    (Some(match_field::MatchType::Lpm), InnerValue::LPM(v, l)) => {
                        assert_eq!(byte_len, v.len());
                        field_match::FieldMatchType::Lpm(
                            crate::proto::p4runtime::field_match::Lpm {
                                value: v.clone(),
                                prefix_len: *l,
                            },
                        )
                    }
                    (Some(match_field::MatchType::Ternary), InnerValue::TERNARY(v, mask)) => {
                        assert_eq!(byte_len, v.len());
                        //                assert_eq!(byte_len, mask.len());
                        let mask = adjust_value(mask.clone(), byte_len);
                        field_match::FieldMatchType::Ternary(
                            crate::proto::p4runtime::field_match::Ternary {
                                value: v.clone(),
                                mask,
                            },
                        )
                    }
                    (Some(match_field::MatchType::Range), InnerValue::RANGE(low, high)) => {
                        assert_eq!(byte_len, low.len());
                        assert_eq!(byte_len, high.len());
                        field_match::FieldMatchType::Range(
                            crate::proto::p4runtime::field_match::Range {
                                low: low.clone(),
                                high: high.clone(),
                            },
                        )
                    }
                    (Some(match_field::MatchType::Ternary), InnerValue::EXACT(v)) => {
                        assert_eq!(byte_len, v.len());
                        //                assert_eq!(byte_len, mask.len());
                        let mask = adjust_value_with(
                            Bytes::from_static(&[0xff, 0xff, 0xff, 0xff]),
                            byte_len,
                            0xff,
                        );
                        for i in 0..byte_len {
                            assert!(mask.as_ref()[i] & v.as_ref()[i] == v.as_ref()[i]);
                        }
                        field_match::FieldMatchType::Ternary(
                            crate::proto::p4runtime::field_match::Ternary {
                                value: v.clone(),
                                mask,
                            },
                        )
                    }
                    other => {
                        dbg!(other);
                        panic!("what")
                    }
                }
            }
            match_field::Match::OtherMatchType(_) => panic!("unsupported"),
        }
    });
    let mut p4runtime_match = crate::proto::p4runtime::FieldMatch {
        field_id: p4info_match.id,
        field_match_type: x,
    };
    return Some(p4runtime_match);
}

pub fn get_action_param_by_name<'a>(
    pipeconf: &'a P4Info,
    action_name: &str,
    param: &str,
) -> Option<&'a action::Param> {
    get_action(pipeconf, action_name).map_or(None, |action| {
        for p in action.params.iter() {
            if &p.name == param {
                return Some(p);
            }
        }
        None
    })
}

pub fn get_packout_egress_port_metaid(p4info: &P4Info) -> Option<u32> {
    p4info
        .controller_packet_metadata
        .iter()
        .find(|p| {
            let pre = p.preamble.as_ref().unwrap();
            pre.name == "packet_out"
        })
        .and_then(|x| {
            x.metadata
                .iter()
                .find(|meta| meta.name == "egress_port")
                .map(|x| x.id)
        })
}

pub fn get_packin_egress_port_metaid(p4info: &P4Info) -> Option<u32> {
    p4info
        .controller_packet_metadata
        .iter()
        .find(|p| {
            let pre = p.preamble.as_ref().unwrap();
            pre.name == "packet_in"
        })
        .and_then(|x| {
            x.metadata
                .iter()
                .find(|meta| meta.name == "ingress_port")
                .map(|x| x.id)
        })
}

pub fn get_action_param_pb(
    pipeconf: &P4Info,
    action_name: &str,
    param_name: &str,
    mut value: InnerParamValue,
) -> crate::proto::p4runtime::action::Param {
    let p4info_param = get_action_param_by_name(pipeconf, action_name, param_name).unwrap();
    let bytes_len = (p4info_param.bitwidth as f32 / 8.0).ceil() as usize;
    //        println!("adjust value: action:{}, param:{}, value:{:?}, bitwidth:{}",action_name,param_name,value,p4info_param.bitwidth);
    let value = adjust_value(value, bytes_len);
    let p4runtime_param = crate::proto::p4runtime::action::Param {
        param_id: p4info_param.id,
        value,
    };
    return p4runtime_param;
}

pub async fn new_set_forwarding_pipeline_config_request(
    p4info: &P4Info,
    bmv2_json_file_path: &Path,
    master_arbitration:&MasterArbitrationUpdate,
    device_id:u64
) -> Result<crate::proto::p4runtime::SetForwardingPipelineConfigRequest, ConnectionError>
{
    let mut file =
        tokio::fs::File::open(bmv2_json_file_path).await.context(ConnectionErrorKind::DeviceConfigFileError)?;
    let mut buffer = String::new();
    file.read_to_string(&mut buffer).await
        .context(ConnectionErrorKind::DeviceConfigFileError)?;
    let election_id = master_arbitration.election_id.clone();
    Ok(crate::proto::p4runtime::SetForwardingPipelineConfigRequest {
        device_id,
        role_id: 0,
        election_id,
        action: crate::proto::p4runtime::set_forwarding_pipeline_config_request::Action::VerifyAndCommit.into(),
        config: Some(crate::proto::p4runtime::ForwardingPipelineConfig {
            p4info: Some(p4info.clone()),
            p4_device_config: buffer.into(),
            cookie: None,
        }),
    })
}

pub fn new_master_update_request(
    device_id:u64,
    option:Bmv2MasterUpdateOption
) -> StreamMessageRequest
{
    StreamMessageRequest {
        update: Some(stream_message_request::Update::Arbitration(
            MasterArbitrationUpdate {
                device_id,
                role: None,
                election_id: Uint128 { high: option.election_id_high, low: option.election_id_low }.into(),
                status: None,
            },
        )),
    }
}

pub fn new_stratum_get_interfaces_name() -> rusty_p4_proto::proto::gnmi::GetRequest {
    rusty_p4_proto::proto::gnmi::GetRequest {
        prefix:None,
        path:vec![crate::gnmi::new_gnmi_path("/interfaces/interface[name=*]/state/name")],
        r#type:1,
        encoding:2,
        use_models:vec![],
        extension:vec![]
    }
}

pub fn new_stratum_get_interface_mac(name:&str) -> rusty_p4_proto::proto::gnmi::GetRequest {
    rusty_p4_proto::proto::gnmi::GetRequest {
        prefix:None,
        path:vec![crate::gnmi::new_gnmi_path(&format!("/interfaces/interface[name={}]/ethernet/state/mac-address",name))],
        r#type:1,
        encoding:2,
        use_models:vec![],
        extension:vec![]
    }
}