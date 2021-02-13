use crate::error::{ConnectionError, ConnectionErrorKind, ContextError};
use crate::failure::ResultExt;
use crate::p4rt::pipeconf::DefaultPipeconf;
use crate::p4rt::pure::adjust_value;
use crate::proto::p4config::P4Info;
use crate::proto::p4runtime::{
    stream_message_request, PacketMetadata, StreamMessageRequest, StreamMessageResponse, TableEntry,
    stream_message_response
};
use crate::{util::{flow::Flow, publisher::{Publisher, Handler}}, representation::{ConnectPoint, DeviceID}, event::PacketReceived, entity::UpdateType};
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
use std::sync::{Arc};
use parking_lot::RwLock;
use futures::{StreamExt, FutureExt, TryFutureExt, TryStreamExt, SinkExt, future::BoxFuture};
use nom::combinator::opt;
use tokio::{sync::mpsc::Sender, io::AsyncReadExt};
use std::{collections::HashMap, convert::TryFrom};
use log::{error, debug};
use super::{pure::{table_entry_to_entity, new_set_entity_request}, pipeconf::Pipeconf};
use async_trait::async_trait;
use crossbeam::atomic::AtomicCell;

type P4RuntimeClient =
    crate::proto::p4runtime::p4_runtime_client::P4RuntimeClient<tonic::transport::channel::Channel>;

#[derive(Clone)]
pub struct Bmv2Manager {
    connections: Arc<RwLock<HashMap<DeviceID, Bmv2SwitchConnection>>>,
    publisher: Arc<crate::util::publisher::Publisher<Bmv2Event>>,
    packet_publisher: Arc<crate::util::publisher::Publisher<PacketReceived>>,
    finish_signal_sender: Arc<crossbeam::atomic::AtomicCell<Option<tokio::sync::oneshot::Sender<()>>>>,
    finish_signal: futures::future::Shared<crate::util::FinishSignal>
}

#[derive(Clone)]
pub enum Bmv2Event {
    DeviceAdded(DeviceID)
}

#[async_trait]
impl crate::app::App for Bmv2Manager {
    type Container = Self;
    type Dependency = ();

    type Option = ();

    fn init<S>(dependencies: Self::Dependency, store: &mut S, option: Self::Option) -> Self where S: crate::app::store::AppStore  {
        let (finish_signal_sender, finish_signal) = tokio::sync::oneshot::channel();
        Bmv2Manager {
            connections: Default::default(),
            publisher: Arc::new(Publisher::default()),
            packet_publisher: Default::default(),
            finish_signal_sender: Arc::new(AtomicCell::new(Some(finish_signal_sender))),
            finish_signal: crate::util::FinishSignal::new(finish_signal).shared()
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
    pub fn subscribe_event<T>(&self, handler: T) where T: Handler<Bmv2Event> {
        self.publisher.add_handler(handler);
    }

    pub fn subscribe_packet<T>(&self, handler: T) where T: Handler<PacketReceived> {
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

    pub async fn add_device<T>(&self, name:&str, address: &str, option: Bmv2ConnectionOption, pipeconf: T)
    where T: Pipeconf + 'static
    {
        let mut device = Bmv2SwitchConnection::try_new(name, address, option).await;
        device.pipeconf = Some(Arc::new(pipeconf));
        let id = device.inner_id;
        let mut sender = device.open_stream().await;
        let mut stream = device.take_channel_receiver().unwrap();
        self.connections.write().insert(id, device);
        let manager = self.clone();
        tokio::spawn(async move {
            while let Some(Ok(r)) = stream.next().await {
                if let Some(update) = r.update {
                    match update {
                        stream_message_response::Update::Arbitration(masterUpdate) => {
                            debug!(target: "core", "StreamMessageResponse?: {:#?}", &masterUpdate);
                            manager.device_master_updated(id).await;
                        }
                        stream_message_response::Update::Packet(packet) => {
                            let x = PacketReceived {
                                packet:packet.payload,
                                from: id,
                                metadata: packet.metadata
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
    }

    pub async fn device_master_updated(&self, device: DeviceID) {
        self.get_device(device).unwrap().get_handle().set_forwarding_pipeline_config().await.unwrap();
        self.publisher.emit(Bmv2Event::DeviceAdded(device)).await;
    }

    pub fn get_device<'a>(&self, device: DeviceID) -> Option<Bmv2SwitchConnection> {
        Some(self.connections.read().get(&device)?.get_handle())
    }

    pub fn get_packet_connectpoint(&self, packet: &PacketReceived) -> Option<ConnectPoint> {
        self.connections.read().get(&packet.from)
        .and_then(|conn| conn.pipeconf.as_ref())
        .and_then(|pipeconf| {
            packet.metadata.iter()
                .find(|x| x.metadata_id == pipeconf.get_packetin_ingress_id())
                .map(|x| BigEndian::read_u16(x.value.as_ref()))
        })
        .map(|port| ConnectPoint {
            device: packet.from,
            port: port as u32,
        })
    }

    pub async fn send_packet(&self, cp: ConnectPoint, bytes: bytes::Bytes) {
        self.connections.read().get(&cp.device).unwrap().get_handle().packet_out(cp.port, bytes).await.unwrap();
    }
}

pub struct Bmv2SwitchConnection {
    pub name: String,
    pub inner_id: DeviceID,
    pub address: String,
    pub device_id: u64,
    pub client: P4RuntimeClient,
    pub conn_status: Bmv2SwitchConnectionStatus,
    pub pipeconf: Option<Arc<dyn Pipeconf>>,
    pub master_arbitration: Option<Bmv2MasterUpdateOption>,
    is_handle: bool
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

#[derive(Copy, Clone)]
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

        let mut client_stub =
            crate::proto::p4runtime::p4_runtime_client::P4RuntimeClient::connect(format!("http://{}",address)).await.unwrap();

        Bmv2SwitchConnection {
            name,
            inner_id: DeviceID(inner_id),
            address,
            device_id,
            client: client_stub,
            conn_status: Bmv2SwitchConnectionStatus::None,
            pipeconf: None,
            is_handle: false,
            master_arbitration: options.master_update
        }
    }

    pub async fn open_stream(&mut self) -> Sender<StreamMessageRequest> {
        if self.is_handle {
            todo!("cannot open stream on handle");
        }
        match self.conn_status {
            Bmv2SwitchConnectionStatus::None => {
               let (mut send_stream, receiver) = tokio::sync::mpsc::channel(4096);
               let master_up_req = crate::p4rt::pure::new_master_update_request(self.device_id,self.master_arbitration.unwrap());
               send_stream.send(master_up_req).await;
               let recv_stream = self.client.stream_channel(tokio_stream::wrappers::ReceiverStream::new(receiver)).await.unwrap().into_inner();
               self.conn_status = Bmv2SwitchConnectionStatus::StreamOpened {
                   sender: send_stream.clone(),
                   receiver: recv_stream,
               };
               send_stream
            }
            Bmv2SwitchConnectionStatus::StreamOpened {
                ref sender,
                ref receiver
            } => {
                sender.clone()
            }
            Bmv2SwitchConnectionStatus::Streaming(ref sender) => {
                unimplemented!()
            }
        }
    }

    pub async fn packet_out(
        &mut self,
        egress_port: u32,
        packet: Bytes,
    ) -> Result<(), ConnectionError> {
        let pipeconf = self.pipeconf.as_ref().unwrap();
        let mut sender = match self.conn_status {
            Bmv2SwitchConnectionStatus::None => {
               let (send_stream, receiver) = tokio::sync::mpsc::channel(4096); 
               let recv_stream = self.client.stream_channel(tokio_stream::wrappers::ReceiverStream::new(receiver)).await.unwrap().into_inner();
               self.conn_status = Bmv2SwitchConnectionStatus::StreamOpened {
                   sender: send_stream.clone(),
                   receiver: recv_stream,
               };
               send_stream
            }
            Bmv2SwitchConnectionStatus::StreamOpened {
                ref sender,
                ref receiver
            } => {
                sender.clone()
            }
            Bmv2SwitchConnectionStatus::Streaming(ref sender) => {
                sender.clone()
            }
        };

        let request = super::pure::new_packet_out_request(&pipeconf, egress_port, packet);
        sender.send(request).await.unwrap();
        Ok(())
    }

    pub async fn set_forwarding_pipeline_config(
        &mut self,
    ) -> Result<(), ConnectionError> 
    {
        let pipeconf = self.pipeconf.as_ref().unwrap();
        let master_arbitration = if let Some(x) = self.master_arbitration { x } else {todo!()};
        let master_arbitration = MasterArbitrationUpdate {
            device_id: self.device_id,
            role: None,
            election_id: Uint128 { high: master_arbitration.election_id_high, low: master_arbitration.election_id_low }.into(),
            status: None,
        };
        let p4info = pipeconf.get_p4info();
        let bmv2_json_file_path = pipeconf.get_bmv2_file_path();
        let request = super::pure::new_set_forwarding_pipeline_config_request(p4info,bmv2_json_file_path,&master_arbitration,self.device_id).await?;
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

    pub fn take_channel_receiver(&mut self) -> Option<tonic::Streaming<rusty_p4_proto::proto::v1::StreamMessageResponse>> {
        let status = std::mem::replace(&mut self.conn_status, Bmv2SwitchConnectionStatus::None);
        let (next_status, ret) = match status {
            Bmv2SwitchConnectionStatus::StreamOpened { sender, receiver } => {
                (Bmv2SwitchConnectionStatus::Streaming(sender),Some(receiver))
            }
            other => {
                (other, None)
            }
        };
        self.conn_status = next_status;
        ret
    }

    pub fn get_handle(&self) -> Self {
        let status = match &self.conn_status {
            Bmv2SwitchConnectionStatus::Streaming(sender) => {
                Bmv2SwitchConnectionStatus::Streaming(sender.clone())
            }
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
            conn_status: status,
            pipeconf: self.pipeconf.clone(),
            master_arbitration: self.master_arbitration.clone(),
            is_handle: true,
        }
    }

    pub async fn set_flow(
        &mut self,
        mut flow: Flow,
        update: UpdateType,
    ) -> Result<Flow, ContextError> {
        let pipeconf = if let Some(pipeconf) = self.pipeconf.as_ref() {
            pipeconf
        }
        else {
            todo!()
        };
        let hash = crate::util::hash(&flow);
        let table_entry = flow.to_table_entry(&pipeconf, hash);
        let request =
            new_set_entity_request(1, table_entry_to_entity(table_entry), update.into());
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

    pub async fn insert_flow(&mut self, mut flow: Flow) -> Result<Flow, ContextError> {
        self.set_flow(flow, UpdateType::Insert).await
    }
}

pub enum Bmv2SwitchConnectionStatus {
    None,
    StreamOpened {
        sender: tokio::sync::mpsc::Sender<rusty_p4_proto::proto::v1::StreamMessageRequest>,
        receiver: tonic::Streaming<rusty_p4_proto::proto::v1::StreamMessageResponse>
    },
    Streaming(tokio::sync::mpsc::Sender<rusty_p4_proto::proto::v1::StreamMessageRequest>)
}