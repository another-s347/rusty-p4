use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use futures03::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures03::sink::SinkExt;
use futures03::stream::StreamExt;
use futures::future::{Future, result};
use futures::sink::Sink;
use futures::stream::Stream;
use grpcio::{StreamingCallSink, WriteFlags};
use tokio::runtime::Runtime;
use tokio::runtime::current_thread::Handle;

use crate::app::p4App;
use crate::error::*;
use crate::p4rt::bmv2::Bmv2SwitchConnection;
use crate::p4rt::helper::P4InfoHelper;
use crate::proto::p4runtime::{PacketIn, StreamMessageRequest, StreamMessageResponse, StreamMessageResponse_oneof_update};
use crate::proto::p4runtime_grpc::P4RuntimeClient;
use crate::event::{PacketReceived, CoreEvent, CoreRequest, Event, CommonEvents};
use crate::util::flow::Flow;
use futures03::future::FutureExt;
use crate::p4rt::pure::{write_table_entry, packet_out_request};
use crate::error::*;
use log::{info, trace, warn, debug, error};
use futures03::compat::*;
use bitfield::fmt::Debug;

mod driver;
mod netconfiguration;

#[derive(Clone)]
pub struct Context<E>
{
    p4info_helper: Arc<P4InfoHelper>,
    pipeconf: String,
    core_channel_sender: UnboundedSender<CoreRequest<E>>,
    event_sender: UnboundedSender<CoreEvent<E>>,
    connections: Arc<RwLock<HashMap<String, Connection>>>,
}

impl<E> Context<E> where E:Event + Clone + 'static + Send
{
    pub async fn try_new<T>(p4info_helper: P4InfoHelper, pipeconfig: String, mut app: T) -> Result<Context<E>>
        where T: p4App<E> + Send + 'static
    {
        let (app_s, app_r) = futures03::channel::mpsc::unbounded();

        let (s, r) = futures03::channel::mpsc::unbounded();

        let mut obj = Context {
            p4info_helper: Arc::new(p4info_helper),
            pipeconf: pipeconfig,
            core_channel_sender: s,
            event_sender: app_s,
            connections: Arc::new(RwLock::new(HashMap::new())),
        };
        let context_handle = obj.get_handle();

        let result = obj.clone();
        let task = r.for_each(move |request| {
            trace!(target:"context","{:#?}",request);
            match request {
                CoreRequest::AddDevice {
                    name,
                    address,
                    device_id,
                    reply
                } => {
                    let bmv2connection = Bmv2SwitchConnection::new(&name, &address, device_id);
                    obj.add_connection(bmv2connection).unwrap();
                }
                CoreRequest::Event(e) => {
                    obj.event_sender.unbounded_send(CoreEvent::Event(e)).unwrap();
                }
                CoreRequest::PacketOut {
                    device,
                    port,
                    packet
                } => {
                    if let Some(c) = obj.connections.write().unwrap().get_mut(&device) {
                        c.pack_out(&obj.p4info_helper, port, packet);
                    }
                    else {
                        error!(target:"context","connection not found for device {}",device);
                    }
                }
            }
            futures03::future::ready(())
        });

        app.on_start(&context_handle);
        tokio::spawn(task);
        tokio::spawn(app_r.for_each(move |x| {
            match x {
                CoreEvent::PacketReceived(packet)=>{
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

    pub fn get_handle(&mut self) -> ContextHandle<E> where E: Event {
        ContextHandle::new(self.p4info_helper.clone(),self.core_channel_sender.clone(), self.connections.clone())
    }

    pub fn add_connection(&mut self, mut connection: Bmv2SwitchConnection) -> Result<()> {
        connection.master_arbitration_update_async()?;
        connection.set_forwarding_pipeline_config(&self.p4info_helper.p4info, Path::new(&self.pipeconf))?;

        let mut packet_s = self.event_sender.clone().compat().sink_map_err(|e| {
            dbg!(e);
        });

        let name = connection.name.clone();
        connection.client.spawn(connection.stream_channel_receiver.for_each(move |x| {
            if let Some(update) = x.update {
                match update {
                    StreamMessageResponse_oneof_update::arbitration(masterUpdate) => {
                        debug!(target:"context", "StreaMessageResponse: {:#?}", masterUpdate);
                    }
                    StreamMessageResponse_oneof_update::packet(packet) => {
                        let x = PacketReceived {
                            packet,
                            from: name.clone()
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
        connection.client.spawn(sink_receiver.forward(connection.stream_channel_sink.sink_map_err(|e|{
            dbg!(e);
        })).map(|_|()));

        self.connections.write().unwrap().insert(connection.name.clone(), Connection {
            p4runtime_client: connection.client,
            sink: sink_sender,
            device_id: connection.device_id
        });

        self.event_sender.start_send(CoreEvent::Event(CommonEvents::DeviceAdded(connection.name).into()));

        Ok(())
    }
}

pub struct Connection {
    pub p4runtime_client: P4RuntimeClient,
    pub sink: futures::sync::mpsc::UnboundedSender<(StreamMessageRequest,WriteFlags)>,
    pub device_id: u64
}

impl Connection {
    pub fn pack_out(&self, p4infoHelper:&P4InfoHelper, port:u32, packet:Vec<u8>) {
        let request = packet_out_request(p4infoHelper, port, packet).unwrap();
        self.sink.unbounded_send(request).unwrap();
    }
}

pub struct ContextHandle<E> {
    p4info_helper: Arc<P4InfoHelper>,
    pub sender: UnboundedSender<CoreRequest<E>>,
    connections: Arc<RwLock<HashMap<String, Connection>>>,
}

impl<E> ContextHandle<E> where E:Debug {
    pub fn new(p4info_helper:Arc<P4InfoHelper>,sender: UnboundedSender<CoreRequest<E>>, connections: Arc<RwLock<HashMap<String, Connection>>>) -> ContextHandle<E> {
        ContextHandle {
            p4info_helper,
            sender,
            connections
        }
    }

    pub fn insert_flow(&self, flow:Flow) -> Result<()> {
        let device = &flow.device;
        let connections = self.connections.read().unwrap();
        let device_client = connections.get(device);
        let table_entry = flow.to_table_entry(self.p4info_helper.as_ref());
        if let Some(connection) = device_client {
            write_table_entry(&connection.p4runtime_client,connection.device_id,table_entry)
        }
        else {
            Err(Box::new(ContextError::DeviceNotConnected(device.clone())))
        }
    }

    pub fn add_device(&self, name:String, address:String, device_id:u64) {
        self.sender.unbounded_send(CoreRequest::AddDevice {
            name,
            address,
            device_id,
            reply:None
        }).unwrap()
    }

    pub fn send_event(&self, event:E) {
        self.sender.unbounded_send(CoreRequest::Event(event)).unwrap();
    }

    pub fn send_packet(&self, device:String, port:u32, packet:Vec<u8>) {
        self.sender.unbounded_send(CoreRequest::PacketOut {
            device,
            port,
            packet
        }).unwrap();
    }
}

