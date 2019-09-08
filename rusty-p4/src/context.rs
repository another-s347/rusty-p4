use crate::app::P4app;
use crate::context::driver::ContextDriver;
use crate::entity::UpdateType;
use crate::error::{ContextError, ContextErrorKind};
use crate::event::{CommonEvents, CoreEvent, CoreRequest, Event, PacketReceived};
use crate::p4rt::bmv2::Bmv2SwitchConnection;
use crate::p4rt::pipeconf::{Pipeconf, PipeconfID};
use crate::p4rt::pure::{new_packet_out_request, new_set_entity_request, new_write_table_entry};
use crate::proto::p4runtime::P4RuntimeClient;
use crate::proto::p4runtime::{
    stream_message_response, Entity, Index, MeterEntry, PacketIn, StreamMessageRequest,
    StreamMessageResponse, Uint128, Update, WriteRequest, WriteResponse,
};
use crate::representation::{ConnectPoint, Device, DeviceID, DeviceType};
use crate::util::flow::Flow;
use byteorder::BigEndian;
use byteorder::ByteOrder;
use bytes::Bytes;
use failure::ResultExt;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::compat::*;
use futures::future::FutureExt;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use futures01::future::Future;
use futures01::sink::Sink;
use futures01::stream::Stream;
use grpcio::{StreamingCallSink, WriteFlags};
use log::{debug, error, info, trace, warn};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use tokio::runtime::current_thread::Handle;
use tokio::runtime::Runtime;

pub mod driver;
pub mod handle;
pub use handle::ContextHandle;
use std::convert::TryInto;

#[derive(Copy, Clone, Default)]
pub struct ContextConfig {
    pub enable_netconfiguration: bool,
}

#[derive(Clone)]
pub struct Context<E> {
    pipeconf: Arc<HashMap<PipeconfID, Pipeconf>>,
    pub core_channel_sender: UnboundedSender<CoreRequest<E>>,
    event_sender: UnboundedSender<CoreEvent<E>>,
    connections: Arc<RwLock<HashMap<DeviceID, Connection>>>,
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
    ) -> Result<(Context<E>, ContextDriver<E, T>), ContextError>
    where
        T: P4app<E> + 'static,
    {
        let (app_s, app_r) = futures::channel::mpsc::unbounded();

        let (s, mut r) = futures::channel::mpsc::unbounded();

        let mut obj = Context {
            pipeconf: Arc::new(pipeconf),
            core_channel_sender: s,
            event_sender: app_s,
            connections: Arc::new(RwLock::new(HashMap::new())),
            id_to_name: Arc::new(RwLock::new(HashMap::new())),
            removed_id_to_name: Arc::new(RwLock::new(HashMap::new())),
            config,
        };
        let context_handle = obj.get_handle();

        let mut result = obj.clone();

        app.on_start(&context_handle);

        let driver = ContextDriver {
            core_request_receiver: r,
            event_receiver: app_r,
            app,
            ctx: obj,
        };

        Ok((result, driver))
    }

    pub fn get_handle(&self) -> ContextHandle<E>
    where
        E: Event,
    {
        ContextHandle::new(
            self.core_channel_sender.clone(),
            self.connections.clone(),
            self.pipeconf.clone(),
        )
    }

    pub async fn add_connection(
        &mut self,
        mut connection: Bmv2SwitchConnection,
        pipeconf: &Pipeconf,
    ) -> Result<(), ContextError> {
        connection
            .master_arbitration_update()
            .context(ContextErrorKind::ConnectionError)?;
        connection
            .set_forwarding_pipeline_config_async(
                pipeconf.get_p4info(),
                pipeconf.get_bmv2_file_path(),
            )
            .await
            .context(ContextErrorKind::ConnectionError)?;

        let mut packet_s = self.event_sender.clone().compat().sink_map_err(|e| {
            dbg!(e);
        });

        let name = connection.name.clone();
        let id = connection.inner_id;
        let packet_in_metaid = pipeconf.packetin_ingress_id;
        let mut event_sender = self.event_sender.clone();
        connection.client.spawn(connection.stream_channel_receiver.for_each(move |x| {
            if let Some(update) = x.update {
                match update {
                    stream_message_response::Update::Arbitration(masterUpdate) => {
                        debug!(target:"context", "StreaMessageResponse: {:#?}", masterUpdate);
                    }
                    stream_message_response::Update::Packet(packet) => {
                        let port = packet.metadata.iter()
                            .find(|x|x.metadata_id==packet_in_metaid)
                            .map(|x|BigEndian::read_u16(x.value.as_ref())).unwrap() as u32;
                        let x = PacketReceived {
                            packet,
                            from: ConnectPoint {
                                device: id,
                                port
                            }
                        };
                        packet_s.start_send(CoreEvent::PacketReceived(x)).unwrap();
                    }
                    stream_message_response::Update::Digest(p) => {
                        debug!(target:"context", "StreaMessageResponse: {:#?}", p);
                    }
                    stream_message_response::Update::IdleTimeoutNotification(n) => {
                        debug!(target:"context", "StreaMessageResponse: {:#?}", n);
                    }
                    stream_message_response::Update::Other(what) => {
                        debug!(target:"context", "StreaMessageResponse: {:#?}", what);
                    }
                }
            }
            Ok(())
        }).map_err(move|e| {
            error!("Connection {}: {:#?}",name,e);
            event_sender.start_send(CoreEvent::Event(CommonEvents::DeviceLost(id).into_e()));
        }));

        let (sink_sender, sink_receiver) = futures01::sync::mpsc::unbounded();
        let error_sender = self.event_sender.clone();
        let mut obj = self.clone();
        connection.client.spawn(
            sink_receiver
                .forward(connection.stream_channel_sink.sink_map_err(move |e| {
                    dbg!(e);
                    error_sender
                        .unbounded_send(CoreEvent::Event(CommonEvents::DeviceLost(id).into_e()));
                    let mut conns = obj.connections.write().unwrap();
                    conns.remove(&id);
                    let mut map = obj.id_to_name.write().unwrap();
                    if let Some(old) = map.remove(&id) {
                        let mut removed_map = obj.id_to_name.write().unwrap();
                        removed_map.insert(id, old);
                    }
                }))
                .map(|_| ()),
        );

        self.connections.write().unwrap().insert(
            id,
            Connection {
                p4runtime_client: connection.client,
                sink: sink_sender,
                device_id: connection.device_id,
                pipeconf: pipeconf.clone(),
            },
        );

        //self.event_sender.start_send(CoreEvent::Event(CommonEvents::DeviceAdded(connection.name).into()));

        Ok(())
    }
}

pub struct Connection {
    pub p4runtime_client: P4RuntimeClient,
    pub sink: futures01::sync::mpsc::UnboundedSender<(StreamMessageRequest, WriteFlags)>,
    pub device_id: u64,
    pub pipeconf: Pipeconf,
}

impl Connection {
    pub fn send_stream_request(&self, request: StreamMessageRequest) -> Result<(), ContextError> {
        self.sink
            .unbounded_send((request, WriteFlags::default()))
            .context(ContextErrorKind::DeviceNotConnected {
                device: DeviceID(self.device_id),
            })?;

        Ok(())
    }

    pub fn send_request_sync(&self, request: &WriteRequest) -> Result<WriteResponse, ContextError> {
        Ok(self
            .p4runtime_client
            .write(request)
            .context(ContextErrorKind::ConnectionError)?)
    }
}
