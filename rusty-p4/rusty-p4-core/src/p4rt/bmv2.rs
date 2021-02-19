use super::{
    pipeconf::Pipeconf,
    pure::{new_set_entity_request, table_entry_to_entity},
};
use crate::proto::p4config::P4Info;
use crate::proto::p4runtime::{
    stream_message_request, stream_message_response, PacketMetadata, StreamMessageRequest,
    StreamMessageResponse, TableEntry,
};
use crate::{
    entity::UpdateType,
    event::PacketReceived,
    representation::{ConnectPoint, DeviceID},
    util::{
        flow::Flow,
        publisher::{Handler, Publisher},
    },
};
use crate::{error::DeviceError, p4rt::pipeconf::DefaultPipeconf};
use crate::{error::InternalError, p4rt::pure::adjust_value};
use async_trait::async_trait;
use byteorder::BigEndian;
use byteorder::ByteOrder;
use bytes::Bytes;
use crossbeam::atomic::AtomicCell;
use futures::{future::BoxFuture, FutureExt, SinkExt, StreamExt, TryFutureExt, TryStreamExt};
use log::{debug, error};
use parking_lot::RwLock;
use prost::Message;
use rusty_p4_proto::proto::v1::{
    Entity, ForwardingPipelineConfig, MasterArbitrationUpdate, Uint128, Update,
};
use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::{collections::HashMap, convert::TryFrom};
use tokio::{io::AsyncReadExt, sync::mpsc::Sender};

type P4RuntimeClient =
    crate::proto::p4runtime::p4_runtime_client::P4RuntimeClient<tonic::transport::channel::Channel>;

#[derive(Clone)]
pub struct Bmv2Manager {
    connections: Arc<RwLock<HashMap<DeviceID, Bmv2SwitchConnection>>>,
    publisher: Arc<crate::util::publisher::Publisher<Bmv2Event>>,
    packet_publisher: Arc<crate::util::publisher::Publisher<PacketReceived>>,
    finish_signal_sender:
        Arc<crossbeam::atomic::AtomicCell<Option<tokio::sync::oneshot::Sender<()>>>>,
    finish_signal: futures::future::Shared<crate::util::FinishSignal>,
}

#[derive(Clone)]
pub enum Bmv2Event {
    DeviceAdded(DeviceID),
}

#[async_trait]
impl crate::app::App for Bmv2Manager {
    type Container = Self;
    type Dependency = ();

    type Option = ();

    const Name: &'static str = "Bmv2Manager";

    fn init<S>(dependencies: Self::Dependency, store: &mut S, option: Self::Option) -> Self
    where
        S: crate::app::store::AppStore,
    {
        let (finish_signal_sender, finish_signal) = tokio::sync::oneshot::channel();
        Bmv2Manager {
            connections: Default::default(),
            publisher: Arc::new(Publisher::default()),
            packet_publisher: Default::default(),
            finish_signal_sender: Arc::new(AtomicCell::new(Some(finish_signal_sender))),
            finish_signal: crate::util::FinishSignal::new(finish_signal).shared(),
        }
    }

    fn from_inner(app: Option<Self::Container>) -> Option<Self> {
        app
    }

    async fn run(&self) {
        self.finish_signal.clone().await;
    }
}

impl Bmv2Manager {
    pub fn subscribe_event<T>(&self, handler: T)
    where
        T: Handler<Bmv2Event>,
    {
        self.publisher.add_handler(handler);
    }

    pub fn subscribe_packet<T>(&self, handler: T)
    where
        T: Handler<PacketReceived>,
    {
        self.packet_publisher.add_handler(handler);
    }

    fn signal_finish(&self) {
        if let Some(sender) = self.finish_signal_sender.swap(None) {
            sender.send(());
        }
    }

    pub async fn del_device(&self, device: DeviceID) {
        let mut conns = self.connections.write();
        conns.remove(&device);
        if conns.is_empty() {
            self.signal_finish();
        }
    }

    pub async fn add_device<T>(
        &self,
        name: &str,
        address: &str,
        option: Bmv2ConnectionOption,
        pipeconf: T,
    ) -> crate::error::Result<()>
    where
        T: Pipeconf + 'static,
    {
        let mut device = Bmv2SwitchConnection::new(name, address, option).await?;
        let pipeconf = Arc::new(pipeconf);
        let id = device.inner_id;
        let mut sender = device.open_stream().await;
        let mut stream = device
            .take_channel_receiver()
            .ok_or_else(|| InternalError::Other {
                err: "take channel receiver failed".to_owned(),
            })?;
        self.connections.write().insert(id, device);
        let manager = self.clone();
        tokio::spawn(async move {
            while let Some(Ok(r)) = stream.next().await {
                if let Some(update) = r.update {
                    match update {
                        stream_message_response::Update::Arbitration(masterUpdate) => {
                            debug!(target: "core", "StreamMessageResponse?: {:#?}", &masterUpdate);
                            // todo: check master update

                            manager.device_master_updated(id, pipeconf.clone()).await;
                        }
                        stream_message_response::Update::Packet(packet) => {
                            let x = PacketReceived {
                                packet: packet.payload,
                                from: id,
                                metadata: packet.metadata,
                            };
                            manager.packet_publisher.emit(x).await;
                        }
                        stream_message_response::Update::Digest(p) => {
                            debug!(target: "core", "StreamMessageResponse: {:#?}", p);
                        }
                        stream_message_response::Update::IdleTimeoutNotification(n) => {
                            debug!(target: "core", "StreamMessageResponse: {:#?}", n);
                        }
                        stream_message_response::Update::Other(what) => {
                            debug!(target: "core", "StreamMessageResponse: {:#?}", what);
                        }
                        stream_message_response::Update::Error(err) => {
                            debug!(target: "core", "StreamMessageResponse: {:#?}", err);
                        }
                    }
                }
            }

            // clean up
            manager.del_device(id).await;
        });

        Ok(())
    }

    pub async fn device_master_updated(
        &self,
        device: DeviceID,
        pipeconf: Arc<dyn Pipeconf>,
    ) -> crate::error::Result<()> {
        self.get_device(device)
            .ok_or(InternalError::DeviceNotFound)?
            .get_handle()
            .set_forwarding_pipeline_config(pipeconf)
            .await?;
        self.publisher.emit(Bmv2Event::DeviceAdded(device)).await;

        Ok(())
    }

    pub fn get_device<'a>(&self, device: DeviceID) -> Option<Bmv2SwitchConnection> {
        Some(self.connections.read().get(&device)?.get_handle())
    }

    pub fn get_packet_connectpoint(&self, packet: &PacketReceived) -> Option<ConnectPoint> {
        self.connections
            .read()
            .get(&packet.from)
            .and_then(|conn| conn.pipeconf.as_ref())
            .and_then(|pipeconf| {
                packet
                    .metadata
                    .iter()
                    .find(|x| x.metadata_id == pipeconf.get_packetin_ingress_id())
                    .map(|x| BigEndian::read_u16(x.value.as_ref()))
            })
            .map(|port| ConnectPoint {
                device: packet.from,
                port: port as u32,
            })
    }

    pub async fn send_packet(
        &self,
        cp: ConnectPoint,
        bytes: bytes::Bytes,
    ) -> crate::error::Result<()> {
        self.connections
            .read()
            .get(&cp.device)
            .ok_or(crate::error::DeviceError::DeviceNotConnected { device: cp.device })?
            .get_handle()
            .packet_out(cp.port, bytes)
            .await?;

        Ok(())
    }
}

/// A connection to bmv2 switch using p4runtime API.
///
/// To connect and use a bmv2 switch, you need to:
/// - make a connection using [Bmv2SwitchConnection::new] with some options [Bmv2ConnectionOption].
/// - open bi-direction stream using [Bmv2SwitchConnection::open_stream],
///   it will send request to acquire the privilage of the switch,
///   open a bi-stream gRPC channel, and returns the send side channel
/// - take the receiver using [Bmv2SwitchConnection::take_channel_receiver], then process the message from switch.
///   If we acquired the privilage successfully, we will receive a msg [stream_message_response::Update::Arbitration],
///   then we can set the pipeline config using [Bmv2SwitchConnection::set_forwarding_pipeline_config].
/// - Done.
pub struct Bmv2SwitchConnection {
    pub name: String,
    pub inner_id: DeviceID,
    pub address: String,
    pub device_id: u64,
    pub client: P4RuntimeClient,
    pub stream_status: Bmv2StreamStatus,
    pipeconf: Option<Arc<dyn Pipeconf>>,
    pub master_status: Bmv2MasterStatus,
    is_handle: bool,
}

pub struct Bmv2ConnectionOption {
    /// the device id used in p4runtime
    pub p4_device_id: u64,
    /// the device id used in rusty_p4, wrapped in [DeviceID].
    /// if this field is None, it will be generated.
    pub inner_device_id: Option<u64>,
    /// the p4runtime election id
    pub master_update: Option<Bmv2MasterUpdateOption>,
}

impl Default for Bmv2ConnectionOption {
    fn default() -> Self {
        Self {
            p4_device_id: 1,
            inner_device_id: None,
            master_update: Some(Bmv2MasterUpdateOption::default()),
        }
    }
}

#[derive(Copy, Clone)]
pub struct Bmv2MasterUpdateOption {
    pub election_id_high: u64,
    pub election_id_low: u64,
}

impl Default for Bmv2MasterUpdateOption {
    fn default() -> Self {
        Bmv2MasterUpdateOption {
            election_id_high: 0,
            election_id_low: 1,
        }
    }
}

impl Bmv2SwitchConnection {
    pub async fn new(
        name: &str,
        address: &str,
        options: Bmv2ConnectionOption,
    ) -> crate::error::Result<Bmv2SwitchConnection> {
        let name = name.to_owned();
        let address = address.to_owned();

        let inner_id = if let Some(inner_id) = options.inner_device_id {
            inner_id
        } else {
            crate::util::hash(&name)
        };
        let device_id = options.p4_device_id;

        let mut client_stub = crate::proto::p4runtime::p4_runtime_client::P4RuntimeClient::connect(
            format!("http://{}", address),
        )
        .await
        .map_err(|e| crate::error::DeviceError::DeviceGrpcTransportError {
            device: DeviceID(inner_id),
            error: e,
        })?;

        Ok(Bmv2SwitchConnection {
            name,
            inner_id: DeviceID(inner_id),
            address,
            device_id,
            client: client_stub,
            stream_status: Bmv2StreamStatus::None,
            pipeconf: None,
            is_handle: false,
            master_status: Bmv2MasterStatus::from(options.master_update),
        })
    }

    pub async fn open_stream(&mut self) -> crate::error::Result<Sender<StreamMessageRequest>> {
        if self.is_handle {
            return Err(DeviceError::Other {
                device: self.inner_id,
                error: "cannot open stream on handle".to_owned(),
            }
            .into());
        }
        match self.stream_status {
            Bmv2StreamStatus::None => {
                let (mut send_stream, receiver) = tokio::sync::mpsc::channel(4096);
                let master_up_req = crate::p4rt::pure::new_master_update_request(
                    self.device_id,
                    self.get_master()?,
                );
                send_stream.send(master_up_req).await;
                let recv_stream = self
                    .client
                    .stream_channel(tokio_stream::wrappers::ReceiverStream::new(receiver))
                    .await
                    .map_err(|e| DeviceError::DeviceGrpcError {
                        device: self.inner_id,
                        error: e,
                    })?
                    .into_inner();
                self.stream_status = Bmv2StreamStatus::StreamOpened {
                    sender: send_stream.clone(),
                    receiver: recv_stream,
                };
                Ok(send_stream)
            }
            Bmv2StreamStatus::StreamOpened {
                ref sender,
                ref receiver,
            } => Ok(sender.clone()),
            Bmv2StreamStatus::Streaming(ref sender) => {
                return Err(DeviceError::Other {
                    device: self.inner_id,
                    error: "already streaming".to_owned(),
                }
                .into());
            }
        }
    }

    pub async fn packet_out(
        &mut self,
        egress_port: u32,
        packet: Bytes,
    ) -> crate::error::Result<()> {
        let pipeconf = self.pipeconf.as_ref().ok_or_else(|| DeviceError::Other {
            device: self.inner_id,
            error: "pipeconf not set".to_owned(),
        })?;
        let mut sender = match self.stream_status {
            Bmv2StreamStatus::None => {
                let (send_stream, receiver) = tokio::sync::mpsc::channel(4096);
                let recv_stream = self
                    .client
                    .stream_channel(tokio_stream::wrappers::ReceiverStream::new(receiver))
                    .await
                    .map_err(|e| crate::error::DeviceError::DeviceGrpcError {
                        device: self.inner_id,
                        error: e,
                    })?
                    .into_inner();
                self.stream_status = Bmv2StreamStatus::StreamOpened {
                    sender: send_stream.clone(),
                    receiver: recv_stream,
                };
                send_stream
            }
            Bmv2StreamStatus::StreamOpened {
                ref sender,
                ref receiver,
            } => sender.clone(),
            Bmv2StreamStatus::Streaming(ref sender) => sender.clone(),
        };

        let request = super::pure::new_packet_out_request(&pipeconf, egress_port, packet);
        sender.send(request).await.unwrap();
        Ok(())
    }

    pub async fn set_forwarding_pipeline_config(
        &mut self,
        pipeconf: Arc<dyn Pipeconf>,
    ) -> crate::error::Result<()> {
        let (e_low, e_high) = self.get_master()?;
        let master_arbitration = MasterArbitrationUpdate {
            device_id: self.device_id,
            role: None,
            election_id: Uint128 {
                high: e_high,
                low: e_low,
            }
            .into(),
            status: None,
        };
        let p4info = pipeconf.get_p4info();
        let bmv2_json_file_path = pipeconf.get_bmv2_file_path();
        let request = super::pure::new_set_forwarding_pipeline_config_request(
            p4info,
            bmv2_json_file_path,
            &master_arbitration,
            self.device_id,
        )
        .await?;
        self.client
            .set_forwarding_pipeline_config(tonic::Request::new(request))
            .await
            .map_err(|e| crate::error::DeviceError::DeviceGrpcError {
                device: self.inner_id,
                error: e,
            })?;

        self.pipeconf = Some(pipeconf);

        Ok(())
    }

    pub async fn write_table_entry(&mut self, table_entry: TableEntry) -> crate::error::Result<()> {
        let (e_low, e_high) = self.get_master()?;
        let update_type = if table_entry.is_default_action {
            crate::proto::p4runtime::update::Type::Modify
        } else {
            crate::proto::p4runtime::update::Type::Insert
        };
        let mut request = crate::proto::p4runtime::WriteRequest {
            device_id: self.device_id,
            role_id: 0,
            election_id: Some(Uint128 {
                high: e_high,
                low: e_low,
            }),
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
            .map_err(|error| crate::error::DeviceError::DeviceGrpcError {
                device: self.inner_id,
                error,
            })?;

        Ok(())
    }

    pub fn take_channel_receiver(
        &mut self,
    ) -> Option<tonic::Streaming<rusty_p4_proto::proto::v1::StreamMessageResponse>> {
        let status = std::mem::replace(&mut self.stream_status, Bmv2StreamStatus::None);
        let (next_status, ret) = match status {
            Bmv2StreamStatus::StreamOpened { sender, receiver } => {
                (Bmv2StreamStatus::Streaming(sender), Some(receiver))
            }
            other => (other, None),
        };
        self.stream_status = next_status;
        ret
    }

    pub fn get_handle(&self) -> Self {
        let status = match &self.stream_status {
            Bmv2StreamStatus::Streaming(sender) => Bmv2StreamStatus::Streaming(sender.clone()),
            _ => {
                todo!()
            }
        };
        Self {
            name: self.name.clone(),
            inner_id: self.inner_id.clone(),
            address: self.address.clone(),
            device_id: self.device_id,
            client: self.client.clone(),
            stream_status: status,
            pipeconf: self.pipeconf.clone(),
            master_status: self.master_status.clone(),
            is_handle: true,
        }
    }

    pub async fn set_flow(
        &mut self,
        mut flow: Flow,
        update: UpdateType,
    ) -> crate::error::Result<Flow> {
        let pipeconf = if let Some(pipeconf) = self.pipeconf.as_ref() {
            pipeconf
        } else {
            todo!()
        };
        let hash = crate::util::hash(&flow);
        let table_entry = flow.to_table_entry(&pipeconf, hash);
        let request = new_set_entity_request(1, table_entry_to_entity(table_entry), update.into());
        match self.client.write(tonic::Request::new(request)).await {
            Ok(response) => {
                debug!(target: "core", "set entity response: {:?}", response);
            }
            Err(e) => {
                error!(target: "core", "grpc send error: {:?}", e);
            }
        }
        flow.metadata = hash;
        Ok(flow)
    }

    pub async fn insert_flow(&mut self, mut flow: Flow) -> crate::error::Result<Flow> {
        self.set_flow(flow, UpdateType::Insert).await
    }

    pub fn get_master(&self) -> crate::error::Result<(u64, u64)> {
        match self.master_status {
            Bmv2MasterStatus::Elect {
                election_id_low,
                election_id_high,
            } => {
                return Err(DeviceError::NotMaster {
                    device: self.inner_id,
                    reason: "Not elected".to_owned(),
                }
                .into());
            }
            Bmv2MasterStatus::Master {
                election_id_low,
                election_id_high,
            } => {
                return Ok((election_id_low, election_id_high));
            }
            Bmv2MasterStatus::NotMaster {} => {
                return Err(DeviceError::NotMaster {
                    device: self.inner_id,
                    reason: "Not master".to_owned(),
                }
                .into());
            }
            Bmv2MasterStatus::NoElect => {
                return Err(DeviceError::NotMaster {
                    device: self.inner_id,
                    reason: "No elect".to_owned(),
                }
                .into());
            }
        }
    }
}

pub enum Bmv2StreamStatus {
    None,
    StreamOpened {
        sender: tokio::sync::mpsc::Sender<rusty_p4_proto::proto::v1::StreamMessageRequest>,
        receiver: tonic::Streaming<rusty_p4_proto::proto::v1::StreamMessageResponse>,
    },
    Streaming(tokio::sync::mpsc::Sender<rusty_p4_proto::proto::v1::StreamMessageRequest>),
}

#[derive(Clone)]
pub enum Bmv2MasterStatus {
    NoElect,
    Elect {
        election_id_low: u64,
        election_id_high: u64,
    },
    Master {
        election_id_low: u64,
        election_id_high: u64,
    },
    NotMaster {},
}

impl Bmv2MasterStatus {
    pub fn from(option: Option<Bmv2MasterUpdateOption>) -> Self {
        match option {
            Some(o) => Self::Elect {
                election_id_low: o.election_id_low,
                election_id_high: o.election_id_high,
            },
            None => Self::NoElect,
        }
    }
}
