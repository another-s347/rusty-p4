use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};
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

use crate::app::P4app;
use crate::core::driver::ContextDriver;
use crate::entity::UpdateType;
use crate::error::{ContextError, ContextErrorKind};
use crate::event::{
    CommonEvents, CoreEvent, CoreRequest, Event, NorthboundRequest, PacketReceived,
};
use crate::p4rt::bmv2::{Bmv2SwitchConnection, Bmv2MasterUpdateOption};
use crate::p4rt::pipeconf::{Pipeconf, PipeconfID};
use crate::p4rt::pure::{new_packet_out_request, new_set_entity_request, new_write_table_entry};
use crate::proto::p4runtime::{
    stream_message_response, Entity, Index, MeterEntry, PacketIn, StreamMessageRequest,
    StreamMessageResponse, Uint128, Update, WriteRequest, WriteResponse,
};
use rusty_p4_proto::proto::v1::{
    MasterArbitrationUpdate
};
use crate::representation::{ConnectPoint, Device, DeviceID, DeviceType};
use crate::util::flow::Flow;
use crate::core::Context;

type P4RuntimeClient =
crate::proto::p4runtime::client::P4RuntimeClient<tonic::transport::channel::Channel>;

#[derive(Copy, Clone, Default)]
pub struct ContextConfig {
    pub enable_netconfiguration: bool,
}

pub struct Core<E> {
    pub(crate) pipeconf: Arc<HashMap<PipeconfID, Pipeconf>>,
    pub(crate) core_channel_sender: UnboundedSender<CoreRequest<E>>,
    pub(crate) event_sender: UnboundedSender<CoreEvent<E>>,
    pub(crate) connections: HashMap<DeviceID, Connection>,
    pub(crate) config: ContextConfig,
}

impl<E> Core<E>
    where
        E: Event + Clone + 'static + Send,
{
    pub async fn try_new<T>(
        pipeconf: HashMap<PipeconfID, Pipeconf>,
        mut app: T,
        config: ContextConfig,
        northbound_channel: Option<UnboundedReceiver<NorthboundRequest>>,
    ) -> Result<(Context<E>, ContextDriver<E, T>), ContextError>
        where
            T: P4app<E> + 'static,
    {
        let (app_s, app_r) = futures::channel::mpsc::unbounded();

        let (s, mut r) = futures::channel::mpsc::unbounded();

        let mut obj = Core {
            pipeconf: Arc::new(pipeconf),
            core_channel_sender: s,
            event_sender: app_s,
            connections:HashMap::new(),
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

    pub fn get_handle(&self) -> Context<E>
        where
            E: Event,
    {
        let conns = self.connections.clone();
        Context::new(
            self.core_channel_sender.clone(),
            conns,
            self.pipeconf.clone(),
        )
    }

    pub async fn add_bmv2_connection(
        &mut self,
        mut connection: Bmv2SwitchConnection,
        pipeconf: &Pipeconf,
    ) -> Result<(), ContextError> {
        let (mut request_sender, request_receiver) = tokio::sync::mpsc::channel(4096);
        let mut client = connection.client.clone();
        let mut event_sender = self.event_sender.clone();
        let id = connection.inner_id;
        tokio::spawn(async move {
            let mut response = client.stream_channel(tonic::Request::new(request_receiver)).await.unwrap().into_inner();
            while let Some(Ok(r)) = response.next().await {
                if let Some(update) = r.update {
                    match update {
                        stream_message_response::Update::Arbitration(masterUpdate) => {
                            debug!(target: "core", "StreaMessageResponse?: {:#?}", &masterUpdate);
                            event_sender.send(CoreEvent::Bmv2MasterUpdate(id,masterUpdate)).await.unwrap();
                        }
                        stream_message_response::Update::Packet(packet) => {
                            let x = PacketReceived {
                                packet:packet.payload,
                                from: id,
                                metadata: packet.metadata
                            };
                            event_sender.send(CoreEvent::PacketReceived(x)).await.unwrap();
                        }
                        stream_message_response::Update::Digest(p) => {
                            debug!(target: "core", "StreaMessageResponse: {:#?}", p);
                        }
                        stream_message_response::Update::IdleTimeoutNotification(n) => {
                            debug!(target: "core", "StreaMessageResponse: {:#?}", n);
                        }
                        stream_message_response::Update::Other(what) => {
                            debug!(target: "core", "StreaMessageResponse: {:#?}", what);
                        }
                    }
                }
            }
        });

        let master_up_req = crate::p4rt::pure::new_master_update_request(connection.device_id,Bmv2MasterUpdateOption::default());
        request_sender.send(master_up_req).await.unwrap();

        let id = connection.inner_id;
        self.connections.insert(id,
                                Connection {
                                    p4runtime_client: connection.client,
                                    sink: request_sender,
                                    device_id: connection.device_id,
                                    pipeconf: pipeconf.clone(),
                                    master_arbitration:None
                                });

        Ok(())
    }
}

#[derive(Clone)]
pub struct Connection {
    pub p4runtime_client: P4RuntimeClient,
    pub sink: tokio::sync::mpsc::Sender<StreamMessageRequest>,
    pub device_id: u64,
    pub pipeconf: Pipeconf,
    pub master_arbitration:Option<MasterArbitrationUpdate>
}

impl Connection {
    pub async fn send_stream_request(
        &mut self,
        request: StreamMessageRequest,
    ) -> Result<(), ContextError> {
        self.sink
            .send(request)
            .await
            .context(ContextErrorKind::DeviceNotConnected {
                device: DeviceID(self.device_id),
            })?;

        Ok(())
    }

    pub async fn send_request(
        &mut self,
        request: WriteRequest,
    ) -> Result<WriteResponse, ContextError> {
        Ok(self
            .p4runtime_client
            .write(tonic::Request::new(request))
            .await
            .context(ContextErrorKind::ConnectionError)?
            .into_inner())
    }

    pub async fn master_up<E>(
        &mut self,
        master_update:MasterArbitrationUpdate,
        context:&mut Context<E>
    ) -> Result<(), ContextError>
        where E:Event+Debug
    {
        self.master_arbitration = Some(master_update);
        let request = crate::p4rt::pure::new_set_forwarding_pipeline_config_request(
            self.pipeconf.get_p4info(),
            self.pipeconf.get_bmv2_file_path(),
            self.master_arbitration.as_ref().unwrap(),
            self.device_id).await.context(ContextErrorKind::ConnectionError)?;
        self.p4runtime_client
            .set_forwarding_pipeline_config(tonic::Request::new(request))
            .await.context(ContextErrorKind::ConnectionError)?;
        context.send_event(CommonEvents::DevicePipeconfUpdate(self.pipeconf.get_id()).into_e());
        Ok(())
    }
}
