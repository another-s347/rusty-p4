use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

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
use crate::util::flow::Flow;
use byteorder::BigEndian;
use byteorder::ByteOrder;
use bytes::Bytes;
use failure::ResultExt;
//use futures::future::{result, Future};
//use futures::sink::Sink;
//use futures::stream::Stream;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::compat::*;
use futures::future::FusedFuture;
use futures::future::Future;
use futures::future::FutureExt;
use futures::select;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use futures::task::Poll;
//use futures03::task::Context;
//use futures03::task::Poll;
use futures::core_reexport::task::Context;
use grpcio::{StreamingCallSink, WriteFlags};
use log::{debug, error, info, trace, warn};
use tokio::runtime::current_thread::Handle;
use tokio::runtime::Runtime;

use super::Context as AppContext;
use super::ContextHandle;

pub struct ContextDriver<E, T> {
    pub core_request_receiver: UnboundedReceiver<CoreRequest<E>>,
    pub event_receiver: UnboundedReceiver<CoreEvent<E>>,
    pub app: T,
    pub ctx: AppContext<E>,
}

impl<E, T> ContextDriver<E, T>
where
    E: Event,
    T: P4app<E>,
{
    async fn run(mut self) {
        let mut ctx = self.ctx;
        let mut handle = ctx.get_handle();
        loop {
            select! {
                request = self.core_request_receiver.next() => {
                    match request {
                        Some(CoreRequest::AddDevice {
                            ref device,
                            reply
                        }) => {
                            if Self::add_device(device, &mut ctx).await {
                                handle = ctx.get_handle();
                            }
                        }
                        Some(CoreRequest::Event(e)) => {
                            ctx.event_sender.send(CoreEvent::Event(e)).await.unwrap();
                        }
                        Some(CoreRequest::RemoveDevice {
                            device
                        }) => {
                            if Self::remove_device(device, &mut ctx).await {
                                handle = ctx.get_handle();
                            }
                        }
                        Some(CoreRequest::PacketOut {
                            connect_point,
                            packet,
                        }) => {
                            if let Some(c) = ctx.connections.read().unwrap().get(&connect_point.device) {
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
                        Some(CoreRequest::SetEntity { device, entity, op }) => {
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
                        _ => {

                        }
                    }
                }
                event = self.event_receiver.next() => {
                    match event {
                        Some(CoreEvent::PacketReceived(packet)) => {
                            self.app.on_packet(packet, &handle);
                        }
                        Some(CoreEvent::Event(e)) => {
                            self.app.on_event(e, &handle);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    async fn add_device(device: &Device, ctx: &mut AppContext<E>) -> bool {
        let name = &device.name;
        match device.typ {
            DeviceType::MASTER {
                ref socket_addr,
                device_id,
                pipeconf,
            } => {
                if ctx.connections.read().unwrap().contains_key(&device.id) {
                    error!(target:"context","Device with name existed: {:?}",device.name);
                    return false;
                }
                let pipeconf_obj = ctx.pipeconf.get(&pipeconf);
                if pipeconf_obj.is_none() {
                    error!(target:"context","pipeconf not found: {:?}",pipeconf);
                    return false;
                }
                let pipeconf = pipeconf_obj.unwrap().clone();
                let bmv2connection =
                    Bmv2SwitchConnection::new(name, socket_addr, device_id, device.id);
                let result = ctx.add_connection(bmv2connection, &pipeconf).await;
                if result.is_err() {
                    error!(target:"context","add connection fail: {:?}",result.err().unwrap());
                    ctx.event_sender.send(CoreEvent::Event(
                        CommonEvents::DeviceLost(device.id).into_e(),
                    ));
                    return false;
                }
            }
            _ => {}
        }
        ctx.event_sender
            .send(CoreEvent::Event(
                CommonEvents::DeviceAdded(device.clone()).into_e(),
            ))
            .await
            .unwrap();
        true
    }

    async fn remove_device(id: DeviceID, ctx: &mut AppContext<E>) -> bool {
        let mut conns = ctx.connections.write().unwrap();
        Arc::get_mut(&mut conns).unwrap().remove(&id);
        let mut map = ctx.id_to_name.write().unwrap();
        if let Some(old) = map.remove(&id) {
            let mut removed_map = ctx.id_to_name.write().unwrap();
            removed_map.insert(id, old);
        }
        ctx.event_sender
            .send(CoreEvent::Event(CommonEvents::DeviceLost(id).into_e()))
            .await
            .unwrap();
        true
    }

    pub async fn run_to_end(self) {
        //        let handle = self.ctx.get_handle();
        //        tokio::spawn(Self::run_request(self.core_request_receiver, self.ctx));
        //        Self::run_event(self.event_receiver, self.app, handle).await;
        self.run().await;
    }
}
