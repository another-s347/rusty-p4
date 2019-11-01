use crate::error::{ConnectionError, ConnectionErrorKind};
use crate::failure::ResultExt;
use crate::p4rt::pipeconf::Pipeconf;
use crate::p4rt::pure::adjust_value;
use crate::proto::p4config::P4Info;
use crate::proto::p4runtime::{
    stream_message_request, PacketMetadata, StreamMessageRequest, StreamMessageResponse, TableEntry,
    stream_message_response
};
use crate::representation::DeviceID;
use byteorder::BigEndian;
use byteorder::ByteOrder;
use bytes::Bytes;
use prost::Message;
use rusty_p4_proto::proto::v1::{
    Entity, ForwardingPipelineConfig, MasterArbitrationUpdate, Uint128, Update,
};
use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use futures::{StreamExt, FutureExt, TryFutureExt, TryStreamExt, SinkExt};
use nom::combinator::opt;
use tokio::io::AsyncReadExt;
use std::convert::TryFrom;
use crate::p4rt::bmv2::Bmv2MasterUpdateOption;

type P4RuntimeClient =
crate::proto::p4runtime::client::P4RuntimeClient<tonic::transport::channel::Channel>;
type GNMIClient = rusty_p4_proto::proto::gnmi::client::GNmiClient<tonic::transport::channel::Channel>;

pub struct StratumBmv2SwitchConnection {
    pub name: String,
    pub inner_id: DeviceID,
    pub address: String,
    pub device_id: u64,
    pub client: P4RuntimeClient,
    pub gnmi_client: GNMIClient
}

pub struct StratumBmv2ConnectionOption {
    pub p4_device_id:u64,
    pub inner_device_id:Option<u64>,
    pub master_update:Option<Bmv2MasterUpdateOption>,
}

impl Default for StratumBmv2ConnectionOption {
    fn default() -> Self {
        Self {
            p4_device_id: 1,
            inner_device_id: None,
            master_update: Some(Bmv2MasterUpdateOption::default())
        }
    }
}

impl StratumBmv2SwitchConnection {
    pub async fn try_new(
        name: &str,
        address: &str,
        options: StratumBmv2ConnectionOption
    ) -> StratumBmv2SwitchConnection {
        let name = name.to_owned();
        let address = address.to_owned();

        let inner_id = if let Some(inner_id) = options.inner_device_id {
            inner_id
        } else {
            crate::util::hash(&name)
        };
        let device_id = options.p4_device_id;

        let mut client_stub = P4RuntimeClient::connect(format!("http://{}",address)).await.unwrap();
        let mut gnmi_client_stub = GNMIClient::connect(format!("http://{}",address)).await.unwrap();

        StratumBmv2SwitchConnection {
            name,
            inner_id: DeviceID(inner_id),
            address,
            device_id,
            client: client_stub,
            gnmi_client: gnmi_client_stub
        }
    }

    pub async fn packet_out(
        &mut self,
        pipeconf: &Pipeconf,
        egress_port: u32,
        packet: Bytes,
    ) -> Result<(), ConnectionError> {
        let request = super::pure::new_packet_out_request(pipeconf, egress_port, packet);
        self.client
            .stream_channel(tonic::Request::new(futures::stream::once(async {
                request
            })))
            .await
            .context(ConnectionErrorKind::GRPCSendError)?;
        Ok(())
    }

    pub async fn set_forwarding_pipeline_config(
        &mut self,
        p4info: &P4Info,
        bmv2_json_file_path: &Path,
        master_arbitration:&MasterArbitrationUpdate
    ) -> Result<(), ConnectionError> {
        let request = super::pure::new_set_forwarding_pipeline_config_request(p4info,bmv2_json_file_path,master_arbitration,self.device_id).await?;
        self.client
            .set_forwarding_pipeline_config(tonic::Request::new(request))
            .await
            .context(ConnectionErrorKind::GRPCSendError)?;

        Ok(())
    }

    pub async fn write_table_entry(&mut self, table_entry: TableEntry) -> Result<(), ConnectionError> {
        let update_type = if table_entry.is_default_action {
            crate::proto::p4runtime::update::Type::Modify
        } else {
            crate::proto::p4runtime::update::Type::Insert
        };
        let mut request = crate::proto::p4runtime::WriteRequest {
            device_id: self.device_id,
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
        self.client
            .write(tonic::Request::new(request))
            .await
            .context(ConnectionErrorKind::GRPCSendError)?;

        Ok(())
    }
}

pub async fn get_interfaces_name(client:&mut GNMIClient) -> Vec<String> {
    let mut v = vec![];
    let response = client.get(super::pure::new_stratum_get_interfaces_name()).await.unwrap();
    for n in &response.get_ref().notification {
        match &n.update[0].val {
            Some(rusty_p4_proto::proto::gnmi::TypedValue {
                    value: Some(rusty_p4_proto::proto::gnmi::typed_value::Value::StringVal(ref name)),
                },
            ) => {
                v.push(name.to_string());
            }
            _ => {}
        };
    }
    v
}
