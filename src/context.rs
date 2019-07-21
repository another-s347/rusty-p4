use std::collections::HashMap;
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
use crate::error::*;
use crate::error::*;
use crate::event::{CommonEvents, CoreEvent, CoreRequest, Event, PacketReceived};
use crate::p4rt::bmv2::Bmv2SwitchConnection;
use crate::p4rt::helper::P4InfoHelper;
use crate::p4rt::pure::{packet_out_request, set_meter_request, write_table_entry};
use crate::proto::p4runtime::{
    Entity, Index, MeterEntry, PacketIn, StreamMessageRequest, StreamMessageResponse,
    StreamMessageResponse_oneof_update, Uint128, Update, Update_Type, WriteRequest,
};
use crate::proto::p4runtime_grpc::P4RuntimeClient;
use crate::representation::{ConnectPoint, Device, DeviceID, DeviceType, Meter};
use crate::util::flow::{Flow, FlowOwned};
use byteorder::BigEndian;
use byteorder::ByteOrder;
use bytes::Bytes;
use futures03::compat::*;
use futures03::future::FutureExt;
use log::{debug, error, info, trace, warn};
use std::fmt::Debug;

mod driver;
mod netconfiguration;

#[derive(Clone)]
pub struct Context<E> {
    p4info_helper: Arc<P4InfoHelper>,
    pipeconf: String,
    core_channel_sender: UnboundedSender<CoreRequest<E>>,
    event_sender: UnboundedSender<CoreEvent<E>>,
    connections: Arc<RwLock<HashMap<DeviceID, Connection>>>,
    id_to_name: Arc<RwLock<HashMap<DeviceID, String>>>,
}

impl<E> Context<E>
where
    E: Event + Clone + 'static + Send,
{
    pub async fn try_new<T>(
        p4info_helper: P4InfoHelper,
        pipeconfig: String,
        mut app: T,
    ) -> Result<Context<E>>
    where
        T: P4app<E> + Send + 'static,
    {
        let (app_s, app_r) = futures03::channel::mpsc::unbounded();

        let (s, r) = futures03::channel::mpsc::unbounded();

        let mut obj = Context {
            p4info_helper: Arc::new(p4info_helper),
            pipeconf: pipeconfig,
            core_channel_sender: s,
            event_sender: app_s,
            connections: Arc::new(RwLock::new(HashMap::new())),
            id_to_name: Arc::new(RwLock::new(HashMap::new())),
        };
        let context_handle = obj.get_handle();

        let result = obj.clone();
        let task = r.for_each(move |request| {
            trace!(target:"context","{:#?}",request);
            match request {
                CoreRequest::AddDevice { device, reply } => {
                    let name = &device.name;
                    match device.typ {
                        DeviceType::MASTER {
                            ref socket_addr,
                            device_id,
                        } => {
                            let bmv2connection =
                                Bmv2SwitchConnection::new(name, socket_addr, device_id, device.id);
                            obj.add_connection(bmv2connection).unwrap();
                        }
                        DeviceType::VIRTUAL {} => {}
                    }
                    obj.event_sender
                        .unbounded_send(CoreEvent::Event(CommonEvents::DeviceAdded(device).into()));
                }
                CoreRequest::Event(e) => {
                    obj.event_sender
                        .unbounded_send(CoreEvent::Event(e))
                        .unwrap();
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
                        c.pack_out(&obj.p4info_helper, connect_point.port, packet);
                    } else {
                        // find device name
                        error!(target:"context","connection not found for device {:?}", connect_point.device);
                    }
                }
                CoreRequest::SetMeter(meter) => {
                    if let Some(c) = obj.connections.write().unwrap().get_mut(&meter.device) {
                        let request = set_meter_request(&obj.p4info_helper, 1, &meter).unwrap();
                        c.p4runtime_client.write(&request);
                    } else {
                        error!(target:"context","connection not found for device {:?}",&meter.device);
                    }
                }
            }
            futures03::future::ready(())
        });

        app.on_start(&context_handle);
        tokio::spawn(task);
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

        netconfiguration::build_netconfig_server(netconfiguration_server, core_sender).await;

        Ok(result)
    }

    pub fn get_handle(&mut self) -> ContextHandle<E>
    where
        E: Event,
    {
        ContextHandle::new(
            self.p4info_helper.clone(),
            self.core_channel_sender.clone(),
            self.connections.clone(),
        )
    }

    pub fn add_connection(&mut self, mut connection: Bmv2SwitchConnection) -> Result<()> {
        connection.master_arbitration_update_async()?;
        connection.set_forwarding_pipeline_config(
            &self.p4info_helper.p4info,
            Path::new(&self.pipeconf),
        )?;

        let mut packet_s = self.event_sender.clone().compat().sink_map_err(|e| {
            dbg!(e);
        });

        let name = connection.name.clone();
        let id = connection.inner_id;
        let packet_in_metaid = self.p4info_helper.packetin_ingress_id;
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
        connection.client.spawn(
            sink_receiver
                .forward(connection.stream_channel_sink.sink_map_err(move |e| {
                    dbg!(e);
                    error_sender
                        .unbounded_send(CoreEvent::Event(CommonEvents::DeviceLost(id).into()));
                }))
                .map(|_| ()),
        );

        self.connections.write().unwrap().insert(
            id,
            Connection {
                p4runtime_client: connection.client,
                sink: sink_sender,
                device_id: connection.device_id,
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
}

impl Connection {
    pub fn pack_out(&self, p4infoHelper: &P4InfoHelper, port: u32, packet: Bytes) {
        let request = packet_out_request(p4infoHelper, port, packet).unwrap();
        self.sink.unbounded_send(request).unwrap();
    }
}

pub struct ContextHandle<E> {
    p4info_helper: Arc<P4InfoHelper>,
    pub sender: UnboundedSender<CoreRequest<E>>,
    connections: Arc<RwLock<HashMap<DeviceID, Connection>>>,
}

impl<E> ContextHandle<E>
where
    E: Debug,
{
    pub fn new(
        p4info_helper: Arc<P4InfoHelper>,
        sender: UnboundedSender<CoreRequest<E>>,
        connections: Arc<RwLock<HashMap<DeviceID, Connection>>>,
    ) -> ContextHandle<E> {
        ContextHandle {
            p4info_helper,
            sender,
            connections,
        }
    }

    pub fn insert_flow(&self, flow: Flow) -> Result<FlowOwned> {
        let device = flow.device;
        let hash = crate::util::hash(&flow);
        let table_entry = flow.to_table_entry(self.p4info_helper.as_ref(), hash);
        let connections = self.connections.read().unwrap();
        let device_client = connections.get(&device);
        if let Some(connection) = device_client {
            write_table_entry(
                &connection.p4runtime_client,
                connection.device_id,
                table_entry,
            )?;
            Ok(flow.into_owned(hash))
        } else {
            Err(Box::new(ContextError::DeviceNotConnected(
                "TODO: Get device name from id".to_string(),
            )))
        }
    }

    pub fn add_device(&self, name: String, address: String, device_id: u64) {
        let id = crate::util::hash(&name);
        let device = Device {
            id: DeviceID(id),
            name,
            ports: Default::default(),
            typ: DeviceType::MASTER {
                socket_addr: address,
                device_id,
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
