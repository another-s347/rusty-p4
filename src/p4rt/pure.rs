use log::{debug, error, info, trace, warn};

use crate::error::{ConnectionError, ConnectionErrorKind};
use crate::p4rt::pipeconf::Pipeconf;
use crate::proto::p4info::{
    Action, Action_Param, MatchField, MatchField_MatchType, Meter, P4Info, Table,
};
use crate::proto::p4runtime::{
    Entity, FieldMatch, MeterEntry, PacketMetadata, PacketOut, StreamMessageRequest,
    StreamMessageRequest_oneof_update, TableEntry, Update, Update_Type, WriteRequest,
};
use crate::proto::p4runtime_grpc::P4RuntimeClient;
use crate::representation::Meter as MeterRep;
use crate::util::value::{InnerParamValue, InnerValue};
use byteorder::BigEndian;
use byteorder::ByteOrder;
use bytes::Bytes;
use failure::ResultExt;
use futures::{Future, Sink};
use grpcio::{Channel, ClientDuplexReceiver, StreamingCallSink, WriteFlags};

pub fn new_write_table_entry(device_id: u64, table_entry: TableEntry) -> WriteRequest {
    let mut request = crate::proto::p4runtime::WriteRequest::new();
    request.set_device_id(device_id);
    request.mut_election_id().low = 1;
    let mut update = crate::proto::p4runtime::Update::new();
    if table_entry.is_default_action {
        update.set_field_type(crate::proto::p4runtime::Update_Type::MODIFY);
    } else {
        update.set_field_type(crate::proto::p4runtime::Update_Type::INSERT);
    }
    update
        .mut_entity()
        .mut_table_entry()
        .clone_from(&table_entry);
    request.updates.push(update);
    request
}

pub fn adjust_value(value: Vec<u8>, bytes_len: usize) -> Vec<u8> {
    let mut value = value.clone();
    if bytes_len < value.len() {
        let (_, v2) = value.split_at(value.len() - bytes_len);
        v2.to_vec()
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
    let mut request = StreamMessageRequest::new();
    let mut packetOut = PacketOut::new();
    packetOut.set_payload(packet.to_vec());
    let mut packetout_metadata = PacketMetadata::new();
    packetout_metadata.set_metadata_id(pipeconf.packetout_egress_id);
    let mut v = vec![0u8; 4];
    BigEndian::write_u32(&mut v, egress_port);
    packetout_metadata.set_value(adjust_value(v, 2));
    packetOut.mut_metadata().push(packetout_metadata);
    request.set_packet(packetOut);
    request
}

pub fn new_set_meter_request(
    pipeconf: &Pipeconf,
    device_id: u64,
    meter: &MeterRep,
) -> Result<WriteRequest, ConnectionError> {
    let mut write_request = WriteRequest::new();
    write_request.set_device_id(device_id);
    write_request.mut_election_id().set_low(1);
    let mut update = Update::new();
    update.set_field_type(Update_Type::MODIFY);
    let mut entity = Entity::new();
    let mut meter_entry = MeterEntry::new();
    let meter_id =
        get_meter_id(pipeconf.get_p4info(), &meter.name).ok_or(ConnectionError::from(
            ConnectionErrorKind::PipeconfError(format!("meter id not found for: {}", &meter.name)),
        ))?;
    meter_entry.set_meter_id(meter_id);
    meter_entry.mut_index().set_index(meter.index);
    meter_entry.mut_config().set_cburst(meter.cburst);
    meter_entry.mut_config().set_cir(meter.cir);
    meter_entry.mut_config().set_pburst(meter.pburst);
    meter_entry.mut_config().set_pir(meter.pir);
    entity.set_meter_entry(meter_entry);
    update.set_entity(entity);
    write_request.mut_updates().push(update);
    Ok(write_request)
}

pub fn build_table_entry(
    p4info: &P4Info,
    table_name: &str,
    match_fields: &[(&str, InnerValue)],
    default_action: bool,
    action_name: &str,
    action_params: &[(&str, InnerParamValue)],
    priority: i32,
    metadata: u64,
) -> TableEntry {
    let mut table_entry = crate::proto::p4runtime::TableEntry::new();
    table_entry.set_table_id(get_table_id(p4info, table_name).unwrap());

    table_entry.set_priority(priority);
    table_entry.set_controller_metadata(metadata);

    for (match_field_name, value) in match_fields {
        let entry = get_match_field_pb(p4info, table_name, match_field_name, value).unwrap();
        table_entry.field_match.push(entry)
    }

    if default_action {
        table_entry.set_is_default_action(true);
    }

    if !action_name.is_empty() {
        let action = table_entry.mut_action().mut_action();
        action.set_action_id(get_actions_id(p4info, action_name).unwrap());
        if !action_params.is_empty() {
            for (field_name, value) in action_params {
                action
                    .params
                    .push(get_action_param_pb(p4info, action_name, field_name, value));
            }
        }
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
    //        println!("{}", bitwidth);
    let byte_len = byte_len as usize;
    let mut p4runtime_match = crate::proto::p4runtime::FieldMatch::new();
    p4runtime_match.set_field_id(p4info_match.id);
    match (p4info_match.get_match_type(), value) {
        (MatchField_MatchType::EXACT, InnerValue::EXACT(v)) => {
            //                assert_eq!(byte_len, v.len());
            let v = adjust_value(v.clone(), byte_len);
            p4runtime_match.mut_exact().value = v;
        }
        (MatchField_MatchType::LPM, InnerValue::LPM(v, l)) => {
            assert_eq!(byte_len, v.len());
            p4runtime_match.mut_lpm().prefix_len = *l;
            p4runtime_match.mut_lpm().value = v.clone();
        }
        (MatchField_MatchType::TERNARY, InnerValue::TERNARY(v, mask)) => {
            assert_eq!(byte_len, v.len());
            //                assert_eq!(byte_len, mask.len());
            let mask = adjust_value(mask.clone(), byte_len);
            p4runtime_match.mut_ternary().value = v.clone();
            p4runtime_match.mut_ternary().mask = mask;
        }
        (MatchField_MatchType::RANGE, InnerValue::RANGE(low, high)) => {
            assert_eq!(byte_len, low.len());
            assert_eq!(byte_len, high.len());
            p4runtime_match.mut_range().low = low.clone();
            p4runtime_match.mut_range().high = high.clone();
        }
        _ => panic!("what"),
    }
    return Some(p4runtime_match);
}

pub fn get_action_param_by_name<'a>(
    pipeconf: &'a P4Info,
    action_name: &str,
    param: &str,
) -> Option<&'a Action_Param> {
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
        .get_controller_packet_metadata()
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
        .get_controller_packet_metadata()
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
    value: &InnerParamValue,
) -> crate::proto::p4runtime::Action_Param {
    let p4info_param = get_action_param_by_name(pipeconf, action_name, param_name).unwrap();
    let mut p4runtime_param = crate::proto::p4runtime::Action_Param::new();
    let mut value = value.clone();
    let bytes_len = (p4info_param.bitwidth as f32 / 8.0).ceil() as usize;
    //        println!("adjust value: action:{}, param:{}, value:{:?}, bitwidth:{}",action_name,param_name,value,p4info_param.bitwidth);
    let value = adjust_value(value, bytes_len);
    p4runtime_param.set_param_id(p4info_param.id);
    p4runtime_param.set_value(value.clone());
    return p4runtime_param;
}
