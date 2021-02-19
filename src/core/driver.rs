use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use crate::app::P4app;
use crate::entity::UpdateType;
use crate::error::{ContextError, ContextErrorKind};
use crate::event::{
    CommonEvents, CoreEvent, CoreRequest, Event, NorthboundRequest, PacketReceived,
};
use crate::p4rt::bmv2::{Bmv2ConnectionOption, Bmv2SwitchConnection};
use crate::p4rt::pipeconf::{Pipeconf, PipeconfID};
use crate::p4rt::pure::{new_packet_out_request, new_set_entity_request, new_write_table_entry};
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
//use futures::future::{result, Future};
//use futures::sink::Sink;
//use futures::stream::Stream;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::future::FusedFuture;
use futures::future::Future;
use futures::future::FutureExt;
use futures::select;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use futures::task::Poll;
//use futures03::task::Context;
//use futures03::task::Poll;
use log::{debug, error, info, trace, warn};

use super::Core as AppContext;
use super::DefaultContext;
use crate::core::context::Context;

type P4RuntimeClient =
    crate::proto::p4runtime::p4_runtime_client::P4RuntimeClient<tonic::transport::channel::Channel>;

pub struct ContextDriver<E, T, C>
where C:Context<E>
{
    pub core_request_receiver: futures::channel::mpsc::Receiver<CoreRequest>,
    pub event_receiver: futures::channel::mpsc::Receiver<CoreEvent<E>>,
    pub request_receiver: futures::channel::mpsc::Receiver<NorthboundRequest>,
    pub app: T,
    pub ctx: AppContext<E, C>,
}

impl<E, T, C> ContextDriver<E, T, C>
where
    E: Event,
    T: P4app<E, C>,
    C: Context<E>
{
    async fn run(mut self) {
        let mut ctx = self.ctx;
        let mut handle = ctx.get_handle();
        loop {
            select! {
                request = self.core_request_receiver.next() => {
                    match request {
                        Some(request) => {
                            if let Some(e) = ctx.process_core_request(request).await {
                                handle = ctx.get_handle();
                                self.app.on_context_update(&mut handle).await;
                                ctx.event_sender.send(CoreEvent::Event(e)).await;
                            }
                        }
                        None => { }
                    }
                }
                event = self.event_receiver.next() => {
                    match event {
                        Some(CoreEvent::PacketReceived(packet)) => {
                            self.app.on_packet(packet, &mut handle).await;
                        }
                        Some(CoreEvent::Event(e)) => {
                            self.app.on_event(e, &mut handle).await;
                        }
                        Some(CoreEvent::Bmv2MasterUpdate(device_id,m)) => {
                            if let Err(e) = ctx.master_up(device_id,m).await {
                                error!(target:"core","{:#?}",e);
                            }
                            else {
                                println!("master up");
                                ctx.event_sender.send(CoreEvent::Event(CommonEvents::DeviceMasterUp(device_id).into_e())).await.unwrap();
                            }
                        }
                        _ => {}
                    }
                }
                northbound_request = self.request_receiver.next() => {
                    match northbound_request {
                        Some(r) => {
                            self.app.on_request(r,&mut handle).await;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    pub async fn run_to_end(self) {
        self.run().await;
    }
}
