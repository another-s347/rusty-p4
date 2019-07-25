use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use crate::error::{ConnectionError, ConnectionErrorKind};
use crate::failure::ResultExt;
use crate::p4rt::pipeconf::Pipeconf;
use crate::p4rt::pure::adjust_value;
use crate::proto::p4config::P4DeviceConfig;
use crate::proto::p4info::P4Info;
use crate::proto::p4runtime::{
    PacketMetadata, StreamMessageRequest, StreamMessageResponse, TableEntry,
};
use crate::proto::p4runtime_grpc::P4RuntimeClient;
use crate::representation::DeviceID;
use byteorder::BigEndian;
use byteorder::ByteOrder;
use bytes::Bytes;
use futures::sink::Sink;
use futures::stream::Stream;
use futures03::compat::*;
use grpcio::{Channel, ClientDuplexReceiver, StreamingCallSink, WriteFlags};
use protobuf::Message;

pub struct Bmv2SwitchConnection {
    pub name: String,
    pub inner_id: DeviceID,
    pub address: String,
    pub device_id: u64,
    pub client: P4RuntimeClient,
    pub stream_channel_sink: StreamingCallSink<StreamMessageRequest>,
    pub stream_channel_receiver: ClientDuplexReceiver<StreamMessageResponse>,
}

impl Bmv2SwitchConnection {
    pub fn new_without_id(name: &str, address: &str, device_id: u64) -> Bmv2SwitchConnection {
        let inner_id = crate::util::hash(name);
        Self::new(name, address, device_id, DeviceID(inner_id))
    }

    pub fn new(
        name: &str,
        address: &str,
        device_id: u64,
        inner_id: DeviceID,
    ) -> Bmv2SwitchConnection {
        let environment = grpcio::EnvBuilder::new().build();
        let channelBuilder = grpcio::ChannelBuilder::new(Arc::new(environment));
        let channel = channelBuilder.connect(address);

        let client_stub = crate::proto::p4runtime_grpc::P4RuntimeClient::new(channel);

        let (stream_channel_sink, stream_channel_receiver) = client_stub.stream_channel().unwrap();

        Bmv2SwitchConnection {
            name: name.to_owned(),
            inner_id,
            address: address.to_owned(),
            device_id,
            client: client_stub,
            stream_channel_sink,
            stream_channel_receiver,
        }
    }

    pub fn master_arbitration_update(&mut self) -> Result<(), ConnectionError> {
        let mut request = StreamMessageRequest::new();
        request.mut_arbitration().device_id = self.device_id;
        request.mut_arbitration().mut_election_id().high = 0;
        request.mut_arbitration().mut_election_id().low = 1;
        self.stream_channel_sink
            .start_send((request, WriteFlags::default()));

        Ok(())
    }

    pub fn packet_out(
        &mut self,
        pipeconf: &Pipeconf,
        egress_port: u32,
        packet: Bytes,
    ) -> Result<(), ConnectionError> {
        let request = super::pure::new_packet_out_request(pipeconf, egress_port, packet);
        self.stream_channel_sink
            .start_send((request, WriteFlags::default()))
            .context(ConnectionErrorKind::GRPCSendError)?;
        Ok(())
    }

    pub fn build_device_config(
        bmv2_json_file_path: &Path,
    ) -> Result<P4DeviceConfig, ConnectionError> {
        let mut device_config = crate::proto::p4config::P4DeviceConfig::new();
        device_config.set_reassign(true);
        let mut file =
            File::open(bmv2_json_file_path).context(ConnectionErrorKind::DeviceConfigFileError)?;
        let mut buffer = String::new();
        file.read_to_string(&mut buffer)
            .context(ConnectionErrorKind::DeviceConfigFileError)?;
        device_config.set_device_data(buffer.into_bytes());
        Ok(device_config)
    }

    pub fn set_forwarding_pipeline_config(
        &mut self,
        p4info: &P4Info,
        bmv2_json_file_path: &Path,
    ) -> Result<(), ConnectionError> {
        let mut request = crate::proto::p4runtime::SetForwardingPipelineConfigRequest::new();
        request.mut_election_id().low = 1;
        request.set_device_id(self.device_id);
        let config = request.mut_config();

        config.mut_p4info().clone_from(p4info);
        config.set_p4_device_config(
            Self::build_device_config(bmv2_json_file_path)?
                .write_to_bytes()
                .context(ConnectionErrorKind::DeviceConfigFileError)?,
        );

        request.set_action(
            crate::proto::p4runtime::SetForwardingPipelineConfigRequest_Action::VERIFY_AND_COMMIT,
        );
        self.client
            .set_forwarding_pipeline_config(&request)
            .context(ConnectionErrorKind::GRPCSendError)?;

        Ok(())
    }

    pub async fn set_forwarding_pipeline_config_async(
        &mut self,
        p4info: &P4Info,
        bmv2_json_file_path: &Path,
    ) -> Result<(), ConnectionError> {
        let mut request = crate::proto::p4runtime::SetForwardingPipelineConfigRequest::new();
        request.mut_election_id().low = 1;
        request.set_device_id(self.device_id);
        let config = request.mut_config();

        config.mut_p4info().clone_from(p4info);
        config.set_p4_device_config(
            Self::build_device_config(bmv2_json_file_path)?
                .write_to_bytes()
                .context(ConnectionErrorKind::DeviceConfigFileError)?,
        );

        request.set_action(
            crate::proto::p4runtime::SetForwardingPipelineConfigRequest_Action::VERIFY_AND_COMMIT,
        );
        self.client
            .set_forwarding_pipeline_config_async(&request)
            .context(ConnectionErrorKind::GRPCSendError)?
            .compat()
            .await
            .context(ConnectionErrorKind::GRPCSendError)?;

        Ok(())
    }

    pub fn write_table_entry(&self, table_entry: TableEntry) -> Result<(), ConnectionError> {
        let mut request = crate::proto::p4runtime::WriteRequest::new();
        request.set_device_id(self.device_id);
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
        self.client
            .write(dbg!(&request))
            .context(ConnectionErrorKind::GRPCSendError)?;

        Ok(())
    }
}
