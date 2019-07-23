use log::{debug, error, info, trace, warn};

use crate::error::{ConnectionError, ConnectionErrorKind};
use crate::p4rt::helper::P4InfoHelper;
use crate::proto::p4info::Meter;
use crate::proto::p4runtime::{
    Entity, MeterEntry, PacketMetadata, PacketOut, StreamMessageRequest,
    StreamMessageRequest_oneof_update, TableEntry, Update, Update_Type, WriteRequest,
};
use crate::proto::p4runtime_grpc::P4RuntimeClient;
use crate::representation::Meter as MeterRep;
use byteorder::BigEndian;
use byteorder::ByteOrder;
use bytes::Bytes;
use failure::ResultExt;
use futures::{Future, Sink};
use grpcio::{Channel, ClientDuplexReceiver, StreamingCallSink, WriteFlags};

pub fn write_table_entry(
    client: &P4RuntimeClient,
    device_id: u64,
    table_entry: TableEntry,
) -> Result<(), ConnectionError> {
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
    //    debug!(target:"write_table_entry", "request: {:#?}", &request);
    client
        .write(&request)
        .context(ConnectionErrorKind::GRPCSendError)?;

    Ok(())
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

pub fn packet_out_request(
    p4info: &P4InfoHelper,
    egress_port: u32,
    packet: Bytes,
) -> (StreamMessageRequest, WriteFlags) {
    let mut request = StreamMessageRequest::new();
    let mut packetOut = PacketOut::new();
    packetOut.set_payload(packet.to_vec());
    let mut packetout_metadata = PacketMetadata::new();
    packetout_metadata.set_metadata_id(p4info.packetout_egress_id);
    let mut v = vec![0u8; 4];
    BigEndian::write_u32(&mut v, egress_port);
    packetout_metadata.set_value(adjust_value(v, 2));
    packetOut.mut_metadata().push(packetout_metadata);
    request.set_packet(packetOut);
    (request, WriteFlags::default())
}

pub fn set_meter_request(
    p4info: &P4InfoHelper,
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
    let meter_id = p4info
        .get_meter_id(&meter.name)
        .ok_or(ConnectionError::from(ConnectionErrorKind::PipeconfError(
            format!("meter id not found for: {}", &meter.name),
        )))?;
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
