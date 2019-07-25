use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use futures::future::{result, Future};
use futures::sink::Sink;
use futures::stream::Stream;
use futures03::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures03::sink::SinkExt;
use futures03::stream::StreamExt;
use grpcio::{StreamingCallSink, WriteFlags};
use tokio::runtime::current_thread::Handle;
use tokio::runtime::Runtime;
use crate::app::P4app;
use crate::error::{ContextErrorKind, ContextError};
use crate::event::{CommonEvents, CoreEvent, CoreRequest, Event, PacketReceived};
use crate::p4rt::bmv2::Bmv2SwitchConnection;
use crate::p4rt::pure::{new_packet_out_request, new_set_meter_request, new_write_table_entry};
use crate::proto::p4runtime::{Entity, Index, MeterEntry, PacketIn, StreamMessageRequest, StreamMessageResponse, StreamMessageResponse_oneof_update, Uint128, Update, Update_Type, WriteRequest, WriteResponse};
use crate::proto::p4runtime_grpc::P4RuntimeClient;
use crate::representation::{ConnectPoint, Device, DeviceID, DeviceType, Meter};
use crate::restore;
use crate::restore::Restore;
use crate::util::flow::{Flow, FlowOwned};
use byteorder::BigEndian;
use byteorder::ByteOrder;
use bytes::Bytes;
use futures03::compat::*;
use futures03::future::FutureExt;
use log::{debug, error, info, trace, warn};
use std::fmt::Debug;
use failure::ResultExt;
use crate::p4rt::pipeconf::{PipeconfID, Pipeconf};

mod driver;
mod netconfiguration;

#[derive(Clone)]
pub struct Context<E> {
    pipeconf: Arc<HashMap<PipeconfID, Pipeconf>>,
    core_channel_sender: UnboundedSender<CoreRequest<E>>,
    event_sender: UnboundedSender<CoreEvent<E>>,
    connections: Arc<RwLock<HashMap<DeviceID, Connection>>>,
    id_to_name: Arc<RwLock<HashMap<DeviceID, String>>>,
    removed_id_to_name: Arc<RwLock<HashMap<DeviceID, String>>>,
    restore: Option<Restore>,
}

impl<E> Context<E>
where
    E: Event + Clone + 'static + Send,
{
    pub async fn try_new<T>(
        pipeconf: HashMap<PipeconfID, Pipeconf>,
        mut app: T,
        restore: Option<Restore>,
    ) -> Result<Context<E>, ContextError>
    where
        T: P4app<E> + Send + 'static,
    {
        let (app_s, app_r) = futures03::channel::mpsc::unbounded();

        let (s, mut r) = futures03::channel::mpsc::unbounded();

        let mut obj = Context {
            pipeconf: Arc::new(pipeconf),
            core_channel_sender: s,
            event_sender: app_s,
            connections: Arc::new(RwLock::new(HashMap::new())),
            id_to_name: Arc::new(RwLock::new(HashMap::new())),
            removed_id_to_name: Arc::new(RwLock::new(HashMap::new())),
            restore,
        };
        let context_handle = obj.get_handle();

        let mut result = obj.clone();

        app.on_start(&context_handle);
        tokio::spawn(async move {
            while let Some(request) = r.next().await {
                trace!(target:"context","{:#?}",request);
                match request {
                    CoreRequest::AddDevice { ref device, reply } => {
                        let name = &device.name;
                        match device.typ {
                            DeviceType::MASTER {
                                ref socket_addr,
                                device_id,
                                pipeconf
                            } => {
                                let pipeconf_obj = obj.pipeconf.get(&pipeconf);
                                if pipeconf_obj.is_none() {
                                    error!(target:"context","pipeconf not found: {:?}",pipeconf);
                                    continue;
                                }
                                let pipeconf = pipeconf_obj.unwrap().clone();
                                let bmv2connection =
                                    Bmv2SwitchConnection::new(name, socket_addr, device_id, device.id);
                                let result = obj.add_connection(bmv2connection, &pipeconf).await;
                                if result.is_err() {
                                    error!(target:"context","add connection fail: {:?}",result.err().unwrap());
                                    continue;
                                }
                                if let Some(r) = obj.restore.as_mut() {
                                    r.add_device(device.clone());
                                }
                            }
                            _ => {}
                        }
                        obj.event_sender
                            .send(CoreEvent::Event(CommonEvents::DeviceAdded(device.clone()).into())).await.unwrap();
                    }
                    CoreRequest::Event(e) => {
                        obj.event_sender
                            .send(CoreEvent::Event(e))
                            .await.unwrap();
                    }
                    CoreRequest::PacketOut {
                        connect_point,
                        packet,
                    } => {
                        if let Some(c) = obj
                            .connections
                            .write()
                            .unwrap()
                            .get_mut(&connect_point.device)
                        {
                            let request = new_packet_out_request(&c.pipeconf, connect_point.port, packet);
                            let result = c.send_stream_request(request);
                            if result.is_err() {
                                error!(target:"context","packet out err {:?}", result.err().unwrap());
                            }
                        } else {
                            // find device name
                            error!(target:"context","connection not found for device {:?}", connect_point.device);
                        }
                    }
                    CoreRequest::SetMeter(meter) => {
                        if let Some(c) = obj.connections.write().unwrap().get_mut(&meter.device) {
                            let request = new_set_meter_request(&c.pipeconf, 1, &meter);
                            if request.is_err() {
                                error!(target:"context","set meter pipeconf error: {:?}",request.err().unwrap());
                                continue;
                            }
                            match c.p4runtime_client.write(&request.unwrap()) {
                                Ok(response)=>{
                                    debug!(target:"context","set meter response: {:?}",response);
                                }
                                Err(e)=>{
                                    error!(target:"context","grpc send error: {:?}",e);
                                }
                            }
                        } else {
                            error!(target:"context","connection not found for device {:?}",&meter.device);
                        }
                    }
                }
            }
        });
        tokio::spawn(app_r.for_each(move |x| {
            match x {
                CoreEvent::PacketReceived(packet) => {
                    app.on_packet(packet, &context_handle);
                }
                CoreEvent::Event(e) => {
                    app.on_event(e, &context_handle);
                }
            }
            futures03::future::ready(())
        }));

        let netconfiguration_server = netconfiguration::NetconfigServer::new();
        let core_sender = result.core_channel_sender.clone();

        let handle = result.get_handle();
        if let Some(r) = result.restore.as_mut() {
            r.restore(handle);
        }

        netconfiguration::build_netconfig_server(netconfiguration_server, core_sender).await;

        Ok(result)
    }

    pub fn get_handle(&mut self) -> ContextHandle<E>
    where
        E: Event,
    {
        ContextHandle::new(
            self.core_channel_sender.clone(),
            self.connections.clone(),
            self.id_to_name.clone(),
            self.removed_id_to_name.clone(),
            self.pipeconf.clone()
        )
    }

    pub async fn add_connection(
        &mut self,
        mut connection: Bmv2SwitchConnection,
        pipeconf:&Pipeconf
    ) -> Result<(), ContextError> {
        connection.master_arbitration_update().context(ContextErrorKind::ConnectionError)?;
        connection.set_forwarding_pipeline_config_async(
            pipeconf.get_p4info(),
            pipeconf.get_bmv2_file_path(),
        ).await.context(ContextErrorKind::ConnectionError)?;

        let mut packet_s = self.event_sender.clone().compat().sink_map_err(|e| {
            dbg!(e);
        });

        let name = connection.name.clone();
        let id = connection.inner_id;
        let packet_in_metaid = pipeconf.packetin_ingress_id;
        connection.client.spawn(connection.stream_channel_receiver.for_each(move |x| {
            if let Some(update) = x.update {
                match update {
                    StreamMessageResponse_oneof_update::arbitration(masterUpdate) => {
                        debug!(target:"context", "StreaMessageResponse: {:#?}", masterUpdate);
                    }
                    StreamMessageResponse_oneof_update::packet(packet) => {
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
                    StreamMessageResponse_oneof_update::digest(p) => {
                        debug!(target:"context", "StreaMessageResponse: {:#?}", p);
                    }
                    StreamMessageResponse_oneof_update::idle_timeout_notification(n) => {
                        debug!(target:"context", "StreaMessageResponse: {:#?}", n);
                    }
                    StreamMessageResponse_oneof_update::other(what) => {
                        debug!(target:"context", "StreaMessageResponse: {:#?}", what);
                }
            }
            }
            Ok(())
        }).map_err(|e| {
            dbg!(e);
        }));

        let (sink_sender, sink_receiver) = futures::sync::mpsc::unbounded();
        let error_sender = self.event_sender.clone();
        let mut obj = self.clone();
        connection.client.spawn(
            sink_receiver
                .forward(connection.stream_channel_sink.sink_map_err(move |e| {
                    dbg!(e);
                    error_sender
                        .unbounded_send(CoreEvent::Event(CommonEvents::DeviceLost(id).into()));
                    let mut conns = obj.connections.write().unwrap();
                    conns.remove(&id);
                    let mut map = obj.id_to_name.write().unwrap();
                    if let Some(old) = map.remove(&id) {
                        let mut removed_map = obj.id_to_name.write().unwrap();
                        removed_map.insert(id, old);
                    }
                    if let Some(r) = obj.restore.as_mut() {
                        r.remove_device(id);
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
                pipeconf: pipeconf.clone()
            },
        );

        //self.event_sender.start_send(CoreEvent::Event(CommonEvents::DeviceAdded(connection.name).into()));

        Ok(())
    }
}

pub struct Connection {
    pub p4runtime_client: P4RuntimeClient,
    pub sink: futures::sync::mpsc::UnboundedSender<(StreamMessageRequest, WriteFlags)>,
    pub device_id: u64,
    pub pipeconf: Pipeconf
}

impl Connection {
//    pub fn packet_out(
//        &self,
//        port: u32,
//        packet: Bytes,
//    ) -> Result<(), ContextError> {
//        let request = new_packet_out_request(&self.pipeconf, port, packet);
//        self.sink.unbounded_send(request).context(ContextErrorKind::DeviceNotConnected { device: DeviceID(self.device_id) })?;
//
//        Ok(())
//    }

    pub fn send_stream_request(&self,request:StreamMessageRequest)-> Result<(), ContextError> {
        self.sink.unbounded_send((request,WriteFlags::default())).context(ContextErrorKind::DeviceNotConnected { device: DeviceID(self.device_id) })?;

        Ok(())
    }

    pub fn send_request_sync(&self,request:&WriteRequest)-> Result<WriteResponse, ContextError> {
        Ok(self.p4runtime_client.write(request).context(ContextErrorKind::ConnectionError)?)
    }
}

pub struct ContextHandle<E> {
    pub sender: UnboundedSender<CoreRequest<E>>,
    pipeconf: Arc<HashMap<PipeconfID,Pipeconf>>,
    connections: Arc<RwLock<HashMap<DeviceID, Connection>>>,
    id_to_name: Arc<RwLock<HashMap<DeviceID, String>>>,
    removed_id_to_name: Arc<RwLock<HashMap<DeviceID, String>>>,
}

impl<E> ContextHandle<E>
where
    E: Debug,
{
    pub fn new(
        sender: UnboundedSender<CoreRequest<E>>,
        connections: Arc<RwLock<HashMap<DeviceID, Connection>>>,
        id_to_name: Arc<RwLock<HashMap<DeviceID, String>>>,
        removed_id_to_name: Arc<RwLock<HashMap<DeviceID, String>>>,
        pipeconf:Arc<HashMap<PipeconfID,Pipeconf>>
    ) -> ContextHandle<E> {
        ContextHandle {
            sender,
            pipeconf,
            connections,
            id_to_name,
            removed_id_to_name,
        }
    }

    pub fn insert_flow(&self, flow: Flow) -> Result<FlowOwned, ContextError> {
        let device = flow.device;
        let hash = crate::util::hash(&flow);
        let connections = self.connections.read().unwrap();
        let connection = connections.get(&device)
            .ok_or(ContextError::from(ContextErrorKind::DeviceNotConnected {device}))?;
        let table_entry = flow.to_table_entry(&connection.pipeconf, hash);
        let request = new_write_table_entry(
            connection.device_id,
            table_entry,
        );
        connection.send_request_sync(&request).context(ContextErrorKind::ConnectionError)?;
        Ok(flow.into_owned(hash))
    }

    pub fn add_device(&self, name: String, address: String, device_id: u64, pipeconf:&str) {
        let id = crate::util::hash(&name);
        let pipeconf = crate::util::hash(pipeconf);
        let device = Device {
            id: DeviceID(id),
            name,
            ports: Default::default(),
            typ: DeviceType::MASTER {
                socket_addr: address,
                device_id,
                pipeconf:PipeconfID(pipeconf)
            },
            device_id,
            index: 0,
        };
        self.sender
            .unbounded_send(CoreRequest::AddDevice {
                device,
                reply: None,
            })
            .unwrap()
    }

    pub fn send_event(&self, event: E) {
        self.sender
            .unbounded_send(CoreRequest::Event(event))
            .unwrap();
    }

    pub fn send_packet(&self, to: ConnectPoint, packet: Bytes) {
        self.sender
            .unbounded_send(CoreRequest::PacketOut {
                connect_point: to,
                packet,
            })
            .unwrap();
    }

    pub fn set_meter(&self, meter: Meter) {
        self.sender
            .unbounded_send(CoreRequest::SetMeter(meter))
            .unwrap();
    }
}
