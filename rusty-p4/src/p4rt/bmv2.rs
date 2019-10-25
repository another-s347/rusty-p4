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

type P4RuntimeClient =
    crate::proto::p4runtime::client::P4RuntimeClient<tonic::transport::channel::Channel>;

pub struct Bmv2SwitchConnection {
    pub name: String,
    pub inner_id: DeviceID,
    pub address: String,
    pub device_id: u64,
    pub client: P4RuntimeClient,
    pub stream_request_sender:tokio::sync::mpsc::Sender<StreamMessageRequest>,
    pub stream_response_receiver:tokio::sync::mpsc::Receiver<StreamMessageResponse>,
    pub master_arbitration:Option<MasterArbitrationUpdate>
}

pub struct Bmv2ConnectionOption {
    pub p4_device_id:u64,
    pub inner_device_id:Option<u64>,
    pub master_update:Option<Bmv2MasterUpdateOption>,
}

impl Default for Bmv2ConnectionOption {
    fn default() -> Self {
        Self {
            p4_device_id: 1,
            inner_device_id: None,
            master_update: Some(Bmv2MasterUpdateOption::default())
        }
    }
}

pub struct Bmv2MasterUpdateOption {
    pub election_id_high:u64,
    pub election_id_low:u64
}

impl Default for Bmv2MasterUpdateOption {
    fn default() -> Self {
        Bmv2MasterUpdateOption {
            election_id_high: 0,
            election_id_low: 1
        }
    }
}

async fn drive_bmv2_with_master_update(
    mut client:P4RuntimeClient,
    request_receiver:tokio::sync::mpsc::Receiver<StreamMessageRequest>,
    mut response_sender:tokio::sync::mpsc::Sender<StreamMessageResponse>,
    m_sender:tokio::sync::oneshot::Sender<MasterArbitrationUpdate>) {
    let mut m_sender = Some(m_sender);
    let mut response = client.stream_channel(tonic::Request::new(request_receiver)).await.unwrap().into_inner();
    while let Some(r) = response.next().await {
        match r {
            Ok(StreamMessageResponse {
                   update: Some(stream_message_response::Update::Arbitration(master_updated))
               }) => {
                if let Some(m_sender) = m_sender.take() {
                    m_sender.send(master_updated);
                }
            }
            Ok(other)=> {
                response_sender.send(other).await;
            }
            Err(err) => {
                println!("ERR:{:?}",err);
            }
        }
    }
    println!("done");
}

async fn drive_bmv2(
    mut client:P4RuntimeClient,
    request_receiver:tokio::sync::mpsc::Receiver<StreamMessageRequest>,
    mut response_sender:tokio::sync::mpsc::Sender<StreamMessageResponse>,
) {
    let mut response = client.stream_channel(tonic::Request::new(request_receiver)).await.unwrap().into_inner();
    response.map_err(|e|()).forward(response_sender.sink_map_err(|e|())).await;
}

impl Bmv2SwitchConnection {
    pub async fn try_new(
        name: &str,
        address: &str,
        options: Bmv2ConnectionOption
    ) -> Bmv2SwitchConnection {
        let name = name.to_owned();
        let address = address.to_owned();

        let inner_id = if let Some(inner_id) = options.inner_device_id {
            inner_id
        } else {
            crate::util::hash(&name)
        };
        let device_id = options.p4_device_id;

        let endpoint = tonic::transport::Endpoint::try_from(format!("http://{}",address)).map(|e|e).unwrap();

        let mut client_stub =
            crate::proto::p4runtime::client::P4RuntimeClient::new(endpoint.channel());

        let (mut request_sender,request_receiver) = tokio::sync::mpsc::channel(4096);
        let (mut response_sender,response_receiver) = tokio::sync::mpsc::channel(4096);

        let master_update = if let Some(master_update) = options.master_update {
            let (m_sender,master_update_receiver) = tokio::sync::oneshot::channel();
            tokio::spawn(drive_bmv2_with_master_update(client_stub.clone(),request_receiver,response_sender,m_sender));

            let request = StreamMessageRequest {
                update: Some(stream_message_request::Update::Arbitration(
                    MasterArbitrationUpdate {
                        device_id: options.p4_device_id,
                        role: None,
                        election_id: Uint128 { high: master_update.election_id_high, low: master_update.election_id_low }.into(),
                        status: None,
                    },
                )),
            };

            request_sender.send(request).await;
            Some(master_update_receiver.await.unwrap())
        }
        else {
            tokio::spawn(drive_bmv2(client_stub.clone(),request_receiver,response_sender));
            None
        };

        Bmv2SwitchConnection {
            name,
            inner_id: DeviceID(inner_id),
            address,
            device_id,
            client: client_stub,
            stream_request_sender: request_sender,
            stream_response_receiver: response_receiver,
            master_arbitration: master_update
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
    ) -> Result<(), ConnectionError> {
        let mut file =
            tokio::fs::File::open(bmv2_json_file_path).await.context(ConnectionErrorKind::DeviceConfigFileError)?;
        let mut buffer = String::new();
        file.read_to_string(&mut buffer).await
            .context(ConnectionErrorKind::DeviceConfigFileError)?;
        let election_id = self.master_arbitration.clone().and_then(|x|x.election_id);
        let request = crate::proto::p4runtime::SetForwardingPipelineConfigRequest {
            device_id: self.device_id,
            role_id: 0,
            election_id,
            action: crate::proto::p4runtime::set_forwarding_pipeline_config_request::Action::VerifyAndCommit.into(),
            config: Some(ForwardingPipelineConfig {
                p4info: Some(p4info.clone()),
                p4_device_config: buffer.into_bytes(),
                cookie: None,
            }),
        };
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
