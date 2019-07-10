use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use futures03::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures03::sink::SinkExt;
use futures03::stream::StreamExt;
use futures::future::Future;
use futures::sink::Sink;
use futures::stream::Stream;
use grpcio::StreamingCallSink;
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
use crate::p4rt::pure::write_table_entry;
use crate::error::*;
use log::{info, trace, warn, debug, error};

mod driver;
mod netconfiguration;

#[derive(Clone)]
pub struct Context<E>
{
    p4info_helper: Arc<P4InfoHelper>,
    pipeconf: String,
    core_channel_sender: UnboundedSender<CoreRequest<E>>,
    event_sender: UnboundedSender<CoreEvent<E>>,
    handle: Handle,
    connections: Arc<RwLock<HashMap<String, Connection>>>,
}

impl<E> Context<E> where E:Event + Clone + 'static + Send
{
    pub fn try_new<T>(p4info_helper: P4InfoHelper, pipeconfig: String, mut app: T) -> Result<(Context<E>, Runtime)>
        where T: p4App<E> + Send + 'static
    {
        let (app_s, app_r) = futures03::channel::mpsc::unbounded();

        let mut rt: tokio::runtime::Runtime = tokio::runtime::Runtime::new()?;
        let (s, r) = futures03::channel::mpsc::unbounded();

        let mut obj = Context {
            p4info_helper: Arc::new(p4info_helper),
            pipeconf: pipeconfig,
            core_channel_sender: s,
            event_sender: app_s,
            handle: rt.handle(),
            connections: Arc::new(RwLock::new(HashMap::new())),
        };
        let context_handle = obj.get_handle();

        let result = obj.clone();
        let task = r.for_each(move |request| {
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
            }
            futures03::future::ready(())
        });

        app.on_start(&context_handle);
        rt.spawn(task);
        rt.spawn(app_r.for_each(move |x| {
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
        netconfiguration::run_netconfig(netconfiguration_server, result.core_channel_sender.clone());

        Ok((result, rt))
    }

    pub fn get_handle(&mut self) -> ContextHandle<E> {
        ContextHandle::new(self.p4info_helper.clone(),self.core_channel_sender.clone(), self.handle.clone(), self.connections.clone())
    }

    pub fn add_connection(&mut self, mut connection: Bmv2SwitchConnection) -> Result<()> {
        connection.master_arbitration_update_async()?;
        connection.set_forwarding_pipeline_config(&self.p4info_helper.p4info, Path::new(&self.pipeconf))?;

        let mut packet_s = self.event_sender.clone().compat().sink_map_err(|e| {
            dbg!(e);
        });

        let name = connection.name.clone();
        connection.client.spawn(connection.stream_channel_receiver.for_each(move |x| {
            debug!(target:"context", "StreaMessageResponse: {:#?}", x);
            if let Some(update) = x.update {
                match update {
                    StreamMessageResponse_oneof_update::arbitration(masterUpdate) => {}
                    StreamMessageResponse_oneof_update::packet(packet) => {
                        let x = PacketReceived {
                            packet,
                            from: name.clone()
                        };
                        packet_s.start_send(CoreEvent::PacketReceived(x)).unwrap();
                    }
                    StreamMessageResponse_oneof_update::digest(_) => {}
                    StreamMessageResponse_oneof_update::idle_timeout_notification(_) => {}
                    StreamMessageResponse_oneof_update::other(_) => {}
                }
            }
            Ok(())
        }).map_err(|e| {
            dbg!(e);
        }));

        self.connections.write().unwrap().insert(connection.name.clone(), Connection {
            p4runtime_client: connection.client,
            sink: Arc::new(connection.stream_channel_sink),
            device_id: connection.device_id
        });

        self.event_sender.start_send(CoreEvent::Event(E::from_common(CommonEvents::DeviceAdded(connection.name))));

        Ok(())
    }
}

pub struct Connection {
    pub p4runtime_client: P4RuntimeClient,
    pub sink: Arc<StreamingCallSink<StreamMessageRequest>>,
    pub device_id: u64
}

pub struct ContextHandle<E> {
    p4info_helper: Arc<P4InfoHelper>,
    sender: UnboundedSender<CoreRequest<E>>,
    pub handle: Arc<Handle>,
    connections: Arc<RwLock<HashMap<String, Connection>>>,
}

impl<E> ContextHandle<E> {
    pub fn new(p4info_helper:Arc<P4InfoHelper>,sender: UnboundedSender<CoreRequest<E>>, handle: Handle, connections: Arc<RwLock<HashMap<String, Connection>>>) -> ContextHandle<E> {
        ContextHandle {
            p4info_helper,
            sender,
            handle: Arc::new(handle),
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

    }
}

