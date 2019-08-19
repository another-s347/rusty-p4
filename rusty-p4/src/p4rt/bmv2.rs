use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use crate::error::{ConnectionError, ConnectionErrorKind};
use crate::failure::ResultExt;
use crate::p4rt::pipeconf::Pipeconf;
use crate::p4rt::pure::adjust_value;
use crate::proto::p4device_config::P4DeviceConfig;
use crate::proto::p4config::P4Info;
use crate::proto::p4runtime::{
    PacketMetadata, StreamMessageRequest, StreamMessageResponse, TableEntry, stream_message_request
};
use crate::proto::p4runtime::P4RuntimeClient;
use crate::representation::DeviceID;
use byteorder::BigEndian;
use byteorder::ByteOrder;
use bytes::Bytes;
use futures::sink::Sink;
use futures::stream::Stream;
use futures03::compat::*;
use grpcio::{Channel, ClientDuplexReceiver, StreamingCallSink, WriteFlags};
use prost::Message;
use rusty_p4_proto::proto::v1::{Uint128, ForwardingPipelineConfig, Update, MasterArbitrationUpdate, Entity};

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

        let client_stub = crate::proto::p4runtime::P4RuntimeClient::new(channel);

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
        let request = StreamMessageRequest {
            update: Some(stream_message_request::Update::Arbitration(MasterArbitrationUpdate {
                device_id: self.device_id,
                role: None,
                election_id: Uint128 {
                    high: 0,
                    low: 1
                }.into(),
                status: None
            }))
        };
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
        let mut file =
            File::open(bmv2_json_file_path).context(ConnectionErrorKind::DeviceConfigFileError)?;
        let mut buffer = String::new();
        file.read_to_string(&mut buffer)
            .context(ConnectionErrorKind::DeviceConfigFileError)?;
        Ok(P4DeviceConfig {
            reassign: true,
            extras: None,
            device_data: buffer.into_bytes(),
        })
    }

    pub fn set_forwarding_pipeline_config(
        &mut self,
        p4info: &P4Info,
        bmv2_json_file_path: &Path,
    ) -> Result<(), ConnectionError> {
        let device_config = Self::build_device_config(bmv2_json_file_path)?;
        let mut device_config_buf = vec![0u8; device_config.encoded_len()];
        device_config.encode(&mut device_config_buf);
        let request = crate::proto::p4runtime::SetForwardingPipelineConfigRequest {
            device_id: self.device_id,
            role_id: 0,
            election_id: Some(Uint128 {
                high: 0,
                low: 1,
            }),
            action: crate::proto::p4runtime::set_forwarding_pipeline_config_request::Action::VerifyAndCommit.into(),
            config: Some(ForwardingPipelineConfig {
                p4info: Some(p4info.clone()),
                p4_device_config: device_config_buf,
                cookie: None,
            }),
        };
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
        let device_config = Self::build_device_config(bmv2_json_file_path)?;
        let mut device_config_buf = vec![0u8; device_config.encoded_len()];
        device_config.encode(&mut device_config_buf);
        let request = crate::proto::p4runtime::SetForwardingPipelineConfigRequest {
            device_id: self.device_id,
            role_id: 0,
            election_id: Some(Uint128 {
                high: 0,
                low: 1,
            }),
            action: crate::proto::p4runtime::set_forwarding_pipeline_config_request::Action::VerifyAndCommit.into(),
            config: Some(ForwardingPipelineConfig {
                p4info: Some(p4info.clone()),
                p4_device_config: device_config_buf,
                cookie: None,
            }),
        };
        self.client
            .set_forwarding_pipeline_config_async(&request)
            .context(ConnectionErrorKind::GRPCSendError)?
            .compat()
            .await
            .context(ConnectionErrorKind::GRPCSendError)?;

        Ok(())
    }

    pub fn write_table_entry(&self, table_entry: TableEntry) -> Result<(), ConnectionError> {
        let update_type = if table_entry.is_default_action {
            crate::proto::p4runtime::update::Type::Modify
        } else {
            crate::proto::p4runtime::update::Type::Insert
        };
        let mut request = crate::proto::p4runtime::WriteRequest {
            device_id: self.device_id,
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
        self.client
            .write(dbg!(&request))
            .context(ConnectionErrorKind::GRPCSendError)?;

        Ok(())
    }
}
