use crate::p4runtime::bmv2::Bmv2SwitchConnection;
use crate::p4runtime::helper::P4InfoHelper;
use std::path::Path;
use crate::app::p4App;
use futures::stream::Stream;
use futures::future::Future;
use futures03::stream::StreamExt;
use futures03::sink::SinkExt;
use futures::sink::Sink;
use crate::error::*;
use crate::proto::p4runtime_grpc::P4RuntimeClient;
use grpcio::StreamingCallSink;
use crate::proto::p4runtime::{StreamMessageRequest, StreamMessageResponse, StreamMessageResponse_oneof_update};
use std::sync::{Arc, Mutex};
use tokio::runtime::{Runtime};
use futures03::channel::mpsc::{UnboundedSender,UnboundedReceiver};
use tokio::runtime::current_thread::Handle;

mod driver;

#[derive(Clone)]
pub struct Context {
    p4runtime_client:P4RuntimeClient,
    p4info_helper: Arc<P4InfoHelper>,
    sink: Arc<StreamingCallSink<StreamMessageRequest>>,
    sender: UnboundedSender<i32>,
    handle: Handle
}

impl Context
{
    pub fn try_new<T>(mut connection: Bmv2SwitchConnection, p4info_helper: P4InfoHelper, pipeconfig: &Path, mut app:T) -> Result<(Context,Runtime)>
        where T:p4App + Send + 'static
    {
        connection.set_forwarding_pipeline_config(&p4info_helper.p4info, pipeconfig);
        connection.master_arbitration_update_async();

        let (packet_s,packet_r) = futures03::channel::mpsc::unbounded();
        let mut packet_s = packet_s.compat().sink_map_err(|e|{
            dbg!(e);
        });

        connection.client.spawn(connection.stream_channel_receiver.for_each(move |x|{
            if let Some(update) = x.update {
                match update {
                    StreamMessageResponse_oneof_update::arbitration(masterUpdate)=>{

                    }
                    StreamMessageResponse_oneof_update::packet(packet) => {
                        packet_s.start_send(packet).unwrap();
                    }
                    StreamMessageResponse_oneof_update::digest(_) => {}
                    StreamMessageResponse_oneof_update::idle_timeout_notification(_) => {}
                    StreamMessageResponse_oneof_update::other(_) => {}
                }
            }
            Ok(())
        }).map_err(|e|{
            dbg!(e);
        }));

        let mut rt: tokio::runtime::Runtime = tokio::runtime::Runtime::new().unwrap();
        let (s,r):(_,UnboundedReceiver<i32>) = futures03::channel::mpsc::unbounded();

        let mut obj = Context {
            p4runtime_client:connection.client,
            p4info_helper:Arc::new(p4info_helper),
            sink: Arc::new(connection.stream_channel_sink),
            sender: s,
            handle: rt.handle()
        };
        let context_handle = obj.get_handle();

        let result = obj.clone();
        let task = r.for_each(move |_| {
            let handle = obj.get_handle();
            futures03::future::ready(())
        });

        app.on_start(&context_handle);
        rt.spawn(task);
        rt.spawn(packet_r.for_each(move|x|{
            app.on_packet(x, &context_handle);
            futures03::future::ready(())
        }));

        Ok((result, rt))
    }

    pub fn get_handle(&mut self) -> ContextHandle {
        ContextHandle::new(self.sender.clone(), self.handle.clone())
    }
}

pub struct ContextHandle {
    sender: UnboundedSender<i32>,
    handle: Handle
}

impl ContextHandle {
    pub fn new(sender: UnboundedSender<i32>, handle:Handle) -> ContextHandle {
        ContextHandle {
            sender,
            handle
        }
    }
}

