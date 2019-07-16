use log::{debug, error, info, trace, warn};

use crate::error::*;
use crate::proto::p4runtime::{TableEntry, StreamMessageRequest,StreamMessageRequest_oneof_update, PacketMetadata, PacketOut};
use crate::proto::p4runtime_grpc::P4RuntimeClient;
use crate::p4rt::helper::P4InfoHelper;
use byteorder::BigEndian;
use grpcio::{Channel, ClientDuplexReceiver, StreamingCallSink, WriteFlags};
use futures::{Sink, Future};
use byteorder::ByteOrder;

pub fn write_table_entry(client:&P4RuntimeClient, device_id:u64, table_entry: TableEntry) -> Result<()> {
    let mut request = crate::proto::p4runtime::WriteRequest::new();
    request.set_device_id(device_id);
    request.mut_election_id().low=1;
    let mut update = crate::proto::p4runtime::Update::new();
    if table_entry.is_default_action {
        update.set_field_type(crate::proto::p4runtime::Update_Type::MODIFY);
    }
    else {
        update.set_field_type(crate::proto::p4runtime::Update_Type::INSERT);
    }
    update.mut_entity().mut_table_entry().clone_from(&table_entry);
    request.updates.push(update);
    debug!(target:"write_table_entry", "request: {:#?}", &request);
    client.write(&request)?;

    Ok(())
}

pub fn adjust_value(value:Vec<u8>, bytes_len:usize) -> Vec<u8> {
    let mut value = value.clone();
    if bytes_len < value.len() {
        let (_, v2)=value.split_at(value.len()-bytes_len);
        v2.to_vec()
    }
    else {
        value.extend(vec![0u8;bytes_len-value.len()]);
        value
    }
}

pub fn packet_out_request(p4info:&P4InfoHelper, egress_port:u32, packet:Vec<u8>) -> Result<(StreamMessageRequest,WriteFlags)> {
    let mut request = StreamMessageRequest::new();
    let mut packetOut = PacketOut::new();
    packetOut.set_payload(packet);
    let mut packetout_metadata = PacketMetadata::new();
    packetout_metadata.set_metadata_id(p4info.packetout_egress_id);
    let mut v = vec![0u8;4];
    BigEndian::write_u32(&mut v, egress_port);
    packetout_metadata.set_value(adjust_value(v, 2));
    packetOut.mut_metadata().push(packetout_metadata);
    request.set_packet(packetOut);
    Ok((request, WriteFlags::default()))
}