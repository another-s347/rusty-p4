use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::fmt::Debug;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use byteorder::BigEndian;
use byteorder::ByteOrder;
use bytes::Bytes;
use failure::ResultExt;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::future::FutureExt;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use log::{debug, error, info, trace, warn};
use tokio::runtime::current_thread::Handle;
use tokio::runtime::Runtime;

pub use handle::ContextHandle;

use crate::app::P4app;
use crate::context::driver::ContextDriver;
use crate::entity::UpdateType;
use crate::error::{ContextError, ContextErrorKind};
use crate::event::{
    CommonEvents, CoreEvent, CoreRequest, Event, NorthboundRequest, PacketReceived,
};
use crate::p4rt::bmv2::Bmv2SwitchConnection;
use crate::p4rt::pipeconf::{Pipeconf, PipeconfID};
use crate::p4rt::pure::{new_packet_out_request, new_set_entity_request, new_write_table_entry};
use crate::proto::p4runtime::{
    Entity, Index, MeterEntry, PacketIn, stream_message_response, StreamMessageRequest,
    StreamMessageResponse, Uint128, Update, WriteRequest, WriteResponse,
};
use crate::representation::{ConnectPoint, Device, DeviceID, DeviceType};
use crate::util::flow::Flow;

pub mod driver;
pub mod handle;

type P4RuntimeClient =
crate::proto::p4runtime::client::P4RuntimeClient<tonic::transport::channel::Channel>;

#[derive(Copy, Clone, Default)]
pub struct ContextConfig {
    pub enable_netconfiguration: bool,
}

pub struct Context<E> {
    pipeconf: Arc<HashMap<PipeconfID, Pipeconf>>,
    pub core_channel_sender: UnboundedSender<CoreRequest<E>>,
    event_sender: UnboundedSender<CoreEvent<E>>,
    connections: RwLock<Arc<HashMap<DeviceID, Connection>>>,
    id_to_name: Arc<RwLock<HashMap<DeviceID, String>>>,
    removed_id_to_name: Arc<RwLock<HashMap<DeviceID, String>>>,
    config: ContextConfig,
}

impl<E> Context<E>
    where
        E: Event + Clone + 'static + Send,
{
    pub async fn try_new<T>(
        pipeconf: HashMap<PipeconfID, Pipeconf>,
        mut app: T,
        config: ContextConfig,
        northbound_channel: Option<UnboundedReceiver<NorthboundRequest>>,
    ) -> Result<(ContextHandle<E>, ContextDriver<E, T>), ContextError>
        where
            T: P4app<E> + 'static,
    {
        let (app_s, app_r) = futures::channel::mpsc::unbounded();

        let (s, mut r) = futures::channel::mpsc::unbounded();

        let mut obj = Context {
            pipeconf: Arc::new(pipeconf),
            core_channel_sender: s,
            event_sender: app_s,
            connections: RwLock::new(Arc::new(HashMap::new())),
            id_to_name: Arc::new(RwLock::new(HashMap::new())),
            removed_id_to_name: Arc::new(RwLock::new(HashMap::new())),
            config,
        };
        let mut context_handle = obj.get_handle();

        app.on_start(&mut context_handle).await;

        let northbound_channel = if let Some(r) = northbound_channel {
            r
        } else {
            let (w, r) = futures::channel::mpsc::unbounded();
            r
        };

        let driver = ContextDriver {
            core_request_receiver: r,
            event_receiver: app_r,
            request_receiver: northbound_channel,
            app,
            ctx: obj,
        };

        Ok((context_handle, driver))
    }

    pub fn get_handle(&self) -> ContextHandle<E>
        where
            E: Event,
    {
        let conns = self.connections.read().unwrap().as_ref().clone();
        ContextHandle::new(
            self.core_channel_sender.clone(),
            conns,
            self.pipeconf.clone(),
        )
    }

    pub async fn add_connection(
        &mut self,
        mut connection: Bmv2SwitchConnection,
        pipeconf: &Pipeconf,
    ) -> Result<(), ContextError> {
        connection
            .set_forwarding_pipeline_config(
                pipeconf.get_p4info(),
                pipeconf.get_bmv2_file_path(),
            )
            .await
            .context(ContextErrorKind::ConnectionError)?;

        let mut packet_s = self.event_sender.clone().sink_map_err(|e| {
            dbg!(e);
        });

        let name = connection.name.clone();
        let id = connection.inner_id;
        let packet_in_metaid = pipeconf.packetin_ingress_id;
        let mut event_sender = self.event_sender.clone();
        let mut response_receiver = connection.stream_response_receiver;
        tokio::spawn(async move {
            while let Some(x) = response_receiver.next().await {
                if let Some(update) = x.update {
                    match update {
                        stream_message_response::Update::Arbitration(masterUpdate) => {
                            debug!(target: "context", "StreaMessageResponse?: {:#?}", masterUpdate);
                        }
                        stream_message_response::Update::Packet(packet) => {
                            let port = packet.metadata.iter()
                                .find(|x| x.metadata_id == packet_in_metaid)
                                .map(|x| BigEndian::read_u16(x.value.as_ref())).unwrap() as u32;
                            let x = PacketReceived {
                                packet,
                                from: ConnectPoint {
                                    device: id,
                                    port,
                                },
                            };
                            packet_s.send(CoreEvent::PacketReceived(x)).await.unwrap();
                        }
                        stream_message_response::Update::Digest(p) => {
                            debug!(target: "context", "StreaMessageResponse: {:#?}", p);
                        }
                        stream_message_response::Update::IdleTimeoutNotification(n) => {
                            debug!(target: "context", "StreaMessageResponse: {:#?}", n);
                        }
                        stream_message_response::Update::Other(what) => {
                            debug!(target: "context", "StreaMessageResponse: {:#?}", what);
                        }
                    }
                }
            }
            println!("done");
        });

        let error_sender = self.event_sender.clone();

        let handle = self.get_handle();

        // TODO: Is this the right way to use Rwlock<Arc<T>> ?
        let mut ptr = self.connections.write().unwrap();
        Arc::make_mut(&mut ptr).insert(
            id,
            Connection {
                p4runtime_client: connection.client,
                sink: connection.stream_request_sender,
                device_id: connection.device_id,
                pipeconf: pipeconf.clone(),
            },
        );

        Ok(())
    }
}

#[derive(Clone)]
pub struct Connection {
    pub p4runtime_client: P4RuntimeClient,
    pub sink: tokio::sync::mpsc::Sender<StreamMessageRequest>,
    pub device_id: u64,
    pub pipeconf: Pipeconf,
}

impl Connection {
    pub async fn send_stream_request(&mut self, request: StreamMessageRequest) -> Result<(), ContextError> {
        self.sink
            .send(request)
            .await
            .context(ContextErrorKind::DeviceNotConnected {
                device: DeviceID(self.device_id),
            })?;

        Ok(())
    }

    pub async fn send_request(&mut self, request: WriteRequest) -> Result<WriteResponse, ContextError> {
        Ok(self
            .p4runtime_client
            .write(tonic::Request::new(request)).await
            .context(ContextErrorKind::ConnectionError)?.into_inner())
    }
}
