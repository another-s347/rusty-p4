use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use byteorder::BigEndian;
use byteorder::ByteOrder;
use bytes::Bytes;
use failure::ResultExt;
use futures::future::{result, Future};
use futures::sink::Sink;
use futures::stream::Stream;
use futures03::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures03::compat::*;
use futures03::future::FutureExt;
use futures03::sink::SinkExt;
use futures03::stream::StreamExt;
use grpcio::{StreamingCallSink, WriteFlags};
use log::{debug, error, info, trace, warn};
use tokio::runtime::current_thread::Handle;
use tokio::runtime::Runtime;

use crate::app::P4app;
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
use crate::restore;
use crate::restore::Restore;
use crate::util::flow::Flow;

use super::Context;
use super::ContextHandle;

pub struct ContextDriver<E, T> {
    pub core_request_receiver: UnboundedReceiver<CoreRequest<E>>,
    pub event_receiver: UnboundedReceiver<CoreEvent<E>>,
    pub app: T,
    pub ctx: Context<E>,
}

impl<E, T> ContextDriver<E, T>
where
    E: Event,
    T: P4app<E>,
{
    async fn run_request(mut r: UnboundedReceiver<CoreRequest<E>>, mut ctx: Context<E>) {
        while let Some(request) = r.next().await {
            trace!(target:"context","{:#?}",request);
            match request {
                CoreRequest::AddDevice { ref device, reply } => {
                    let name = &device.name;
                    match device.typ {
                        DeviceType::MASTER {
                            ref socket_addr,
                            device_id,
                            pipeconf,
                        } => {
                            let pipeconf_obj = ctx.pipeconf.get(&pipeconf);
                            if pipeconf_obj.is_none() {
                                error!(target:"context","pipeconf not found: {:?}",pipeconf);
                                continue;
                            }
                            let pipeconf = pipeconf_obj.unwrap().clone();
                            let bmv2connection =
                                Bmv2SwitchConnection::new(name, socket_addr, device_id, device.id);
                            let result = ctx.add_connection(bmv2connection, &pipeconf).await;
                            if result.is_err() {
                                error!(target:"context","add connection fail: {:?}",result.err().unwrap());
                                continue;
                            }
                            if let Some(r) = ctx.restore.as_mut() {
                                r.add_device(device.clone());
                            }
                        }
                        _ => {}
                    }
                    ctx.event_sender
                        .send(CoreEvent::Event(
                            CommonEvents::DeviceAdded(device.clone()).into(),
                        ))
                        .await
                        .unwrap();
                }
                CoreRequest::Event(e) => {
                    ctx.event_sender.send(CoreEvent::Event(e)).await.unwrap();
                }
                CoreRequest::PacketOut {
                    connect_point,
                    packet,
                } => {
                    if let Some(c) = ctx
                        .connections
                        .write()
                        .unwrap()
                        .get_mut(&connect_point.device)
                    {
                        let request =
                            new_packet_out_request(&c.pipeconf, connect_point.port, packet);
                        let result = c.send_stream_request(request);
                        if result.is_err() {
                            error!(target:"context","packet out err {:?}", result.err().unwrap());
                        }
                    } else {
                        // find device name
                        error!(target:"context","PacketOut error: connection not found for device {:?}.", connect_point.device);
                    }
                }
                CoreRequest::SetEntity { device, entity, op } => {
                    if let Some(c) = ctx.connections.read().unwrap().get(&device) {
                        let request = new_set_entity_request(1, entity, op.into());
                        match c.p4runtime_client.write(&request) {
                            Ok(response) => {
                                debug!(target:"context","set entity response: {:?}",response);
                            }
                            Err(e) => {
                                error!(target:"context","grpc send error: {:?}",e);
                            }
                        }
                    } else {
                        error!(target:"context","SetEntity error: connection not found for device {:?}",&device);
                    }
                }
            }
        }
    }

    async fn run_event(mut r: UnboundedReceiver<CoreEvent<E>>, mut app: T, ctx: ContextHandle<E>) {
        while let Some(x) = r.next().await {
            match x {
                CoreEvent::PacketReceived(packet) => {
                    app.on_packet(packet, &ctx);
                }
                CoreEvent::Event(e) => {
                    app.on_event(e, &ctx);
                }
            }
        }
    }

    pub async fn run_to_end(self) {
        let handle = self.ctx.get_handle();
        tokio::spawn(Self::run_request(self.core_request_receiver, self.ctx));
        Self::run_event(self.event_receiver, self.app, handle).await;
    }
}
