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
use crate::event::{PacketReceived, CoreEvent};
use crate::util::flow::Flow;

mod driver;

#[derive(Clone)]
pub struct Context {
    p4info_helper: Arc<P4InfoHelper>,
    pipeconf: String,
    core_channel_sender: UnboundedSender<i32>,
    event_sender: UnboundedSender<CoreEvent>,
    handle: Handle,
    connections: Arc<RwLock<HashMap<String, Connection>>>,
}

impl Context
{
    pub fn try_new<T>(p4info_helper: P4InfoHelper, pipeconfig: String, mut app: T) -> Result<(Context, Runtime)>
        where T: p4App + Send + 'static
    {
        let (app_s, app_r) = futures03::channel::mpsc::unbounded();

        let mut rt: tokio::runtime::Runtime = tokio::runtime::Runtime::new()?;
        let (s, r): (_, UnboundedReceiver<i32>) = futures03::channel::mpsc::unbounded();

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
        let task = r.for_each(move |_| {
            let handle = obj.get_handle();
            futures03::future::ready(())
        });

        app.on_start(&context_handle);
        rt.spawn(task);
        rt.spawn(app_r.for_each(move |x| {
            match x {
                CoreEvent::PacketReceived(packet)=>{
                    app.on_packet(packet, &context_handle);
                }
                CoreEvent::DeviceAdded(s) => {
                    app.on_device(s, &context_handle);
                }
            }
            futures03::future::ready(())
        }));

        Ok((result, rt))
    }

    pub fn get_handle(&mut self) -> ContextHandle {
        ContextHandle::new(self.core_channel_sender.clone(), self.handle.clone(), self.connections.clone())
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
        });

        self.event_sender.start_send(CoreEvent::DeviceAdded(connection.name));

        Ok(())
    }
}

pub struct Connection {
    pub p4runtime_client: P4RuntimeClient,
    pub sink: Arc<StreamingCallSink<StreamMessageRequest>>,
}

pub struct ContextHandle {
    sender: UnboundedSender<i32>,
    pub handle: Handle,
    connections: Arc<RwLock<HashMap<String, Connection>>>,
}

impl ContextHandle {
    pub fn new(sender: UnboundedSender<i32>, handle: Handle, connections: Arc<RwLock<HashMap<String, Connection>>>) -> ContextHandle {
        ContextHandle {
            sender,
            handle,
            connections
        }
    }

    pub fn insert_flow(&self, flow:Flow) {

    }
}

