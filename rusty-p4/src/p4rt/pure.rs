use log::{debug, error, info, trace, warn};

use crate::error::{ConnectionError, ConnectionErrorKind};
use crate::p4rt::pipeconf::Pipeconf;
use crate::proto::p4config::*;
use crate::proto::p4config::P4Info;
use crate::representation::Meter as MeterRep;
use crate::util::flow::{FlowActionParam, FlowMatch};
use crate::util::value::{Encode, InnerParamValue, InnerValue};
use byteorder::BigEndian;
use byteorder::ByteOrder;
use bytes::Bytes;
use failure::ResultExt;
use futures::{Future, Sink};
use grpcio::{Channel, ClientDuplexReceiver, StreamingCallSink, WriteFlags};
use crate::proto::p4runtime::{TableEntry,WriteRequest,StreamMessageRequest,FieldMatch,field_match,stream_message_request};
use rusty_p4_proto::proto::v1::{PacketOut, PacketMetadata, Uint128, Update, Entity, MeterEntry, MeterConfig, Index, TableAction};

pub fn new_write_table_entry(device_id: u64, table_entry: TableEntry) -> WriteRequest {
    let update_type = if table_entry.is_default_action {
        crate::proto::p4runtime::update::Type::Modify
    } else {
        crate::proto::p4runtime::update::Type::Insert
    };
    let mut request = crate::proto::p4runtime::WriteRequest {
        device_id,
        role_id: 0,
        election_id: Some(Uint128 {
            high: 0,
            low: 1
        }),
        updates: vec![Update {
            r#type: update_type as i32,
            entity: Some(Entity {
                entity: Some(crate::proto::p4runtime::entity::Entity::TableEntry(table_entry.clone()))
            })
        }],
        atomicity: 0
    };
    request
}

pub fn adjust_value(mut value: Bytes, bytes_len: usize) -> Bytes {
    if bytes_len == value.len() {
        value
    } else if bytes_len < value.len() {
        value.slice(value.len() - bytes_len, value.len())
    } else {
        value.extend(vec![0u8; bytes_len - value.len()]);
        value
    }
}

pub fn new_packet_out_request(
    pipeconf: &Pipeconf,
    egress_port: u32,
    packet: Bytes,
) -> StreamMessageRequest {
    let packetOut = PacketOut {
        payload: packet.to_vec(),
        metadata: vec![PacketMetadata {
            metadata_id: pipeconf.packetout_egress_id,
            value: adjust_value(Bytes::from(egress_port.to_be_bytes().as_ref()), 2).to_vec()
        }]
    };
    let request = StreamMessageRequest {
        update: Some(stream_message_request::Update::Packet(packetOut))
    };
    request
}

pub fn new_set_meter_request(
    pipeconf: &Pipeconf,
    device_id: u64,
    meter: &MeterRep,
) -> Result<WriteRequest, ConnectionError> {
    let write_request: WriteRequest = WriteRequest {
        device_id,
        role_id: 0,
        election_id: Some(Uint128 {
            high: 0,
            low: 1
        }),
        updates: vec![Update {
            r#type: crate::proto::p4runtime::update::Type::Modify as i32,
            entity: Some(Entity {
                entity: Some(crate::proto::p4runtime::entity::Entity::MeterEntry(MeterEntry {
                    meter_id: get_meter_id(pipeconf.get_p4info(), &meter.name).ok_or(ConnectionError::from(
                        ConnectionErrorKind::PipeconfError(format!("meter id not found for: {}", &meter.name)),
                    ))?,
                    index: Some(Index {
                        index: meter.index
                    }),
                    config: Some(MeterConfig {
                        cir: meter.cir,
                        cburst: meter.cburst,
                        pir: meter.pir,
                        pburst: meter.pburst
                    })
                }))
            })
        }],
        atomicity: 0
    };
    Ok(write_request)
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
        let mut action = crate::proto::p4runtime::Action {
            action_id: get_actions_id(p4info, action_name).unwrap(),
            params: vec![]
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
    }
    else {
        None
    };

    let mut table_entry = TableEntry {
        table_id: get_table_id(p4info, table_name).unwrap(),
        r#match: vec![],
        action: Some(TableAction {
            r#type:action.map(crate::proto::p4runtime::table_action::Type::Action)
        }),
        priority,
        controller_metadata: metadata,
        meter_config: None,
        counter_data: None,
        is_default_action: default_action,
        idle_timeout_ns: 0,
        time_since_last_hit: None
    };

    for m in match_fields {
        let entry = get_match_field_pb(p4info, table_name, m.name, &m.value).unwrap();
        table_entry.r#match.push(entry)
    }

    return table_entry;
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
    //        self.p4info.meters.iter().for_each(|t|{
    //            println!("{}",t.preamble.as_ref().unwrap().name);
    //        });
    get_meter(pipeconf, name).map(|table| table.preamble.as_ref().unwrap().id)
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
    let byte_len = (bitwidth as f32 / 8.0).ceil() as usize;;
    let byte_len = byte_len as usize;
    let x=p4info_match.r#match.as_ref().map(|x|{
        match x {
            match_field::Match::MatchType(x) => {
                match (match_field::MatchType::from_i32(*x),value) {
                    (Some(match_field::MatchType::Exact), InnerValue::EXACT(v)) => {
                        //                assert_eq!(byte_len, v.len());
                        let v = adjust_value(v.clone(), byte_len);
                        field_match::FieldMatchType::Exact(crate::proto::p4runtime::field_match::Exact {
                            value: v.to_vec()
                        })
                    }
                    (Some(match_field::MatchType::Lpm), InnerValue::LPM(v, l)) => {
                        assert_eq!(byte_len, v.len());
                        field_match::FieldMatchType::Lpm(crate::proto::p4runtime::field_match::Lpm {
                            value: v.to_vec(),
                            prefix_len: *l
                        })
                    }
                    (Some(match_field::MatchType::Ternary), InnerValue::TERNARY(v, mask)) => {
                        assert_eq!(byte_len, v.len());
                        //                assert_eq!(byte_len, mask.len());
                        let mask = adjust_value(mask.clone(), byte_len);
                        field_match::FieldMatchType::Ternary(crate::proto::p4runtime::field_match::Ternary {
                            value: v.to_vec(),
                            mask: mask.to_vec()
                        })
                    }
                    (Some(match_field::MatchType::Range), InnerValue::RANGE(low, high)) => {
                        assert_eq!(byte_len, low.len());
                        assert_eq!(byte_len, high.len());
                        field_match::FieldMatchType::Range(crate::proto::p4runtime::field_match::Range {
                        low: low.to_vec(),
                        high: high.to_vec()
                        })
                    }
                    _=>{
                        panic!("what")
                    }
                }
            }
            match_field::Match::OtherMatchType(_)=>{
                panic!("unsupported")
            }
        }
    });
    let mut p4runtime_match = crate::proto::p4runtime::FieldMatch {
        field_id: p4info_match.id,
        field_match_type: x
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
        .map(|x| {
            x.metadata
                .iter()
                .find(|meta| meta.name == "egress_port")
                .map(|x| x.id)
        })
        .flatten()
}

pub fn get_packin_egress_port_metaid(p4info: &P4Info) -> Option<u32> {
    p4info
        .controller_packet_metadata
        .iter()
        .find(|p| {
            let pre = p.preamble.as_ref().unwrap();
            pre.name == "packet_in"
        })
        .map(|x| {
            x.metadata
                .iter()
                .find(|meta| meta.name == "ingress_port")
                .map(|x| x.id)
        })
        .flatten()
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
        value: value.to_vec()
    };
    return p4runtime_param;
}