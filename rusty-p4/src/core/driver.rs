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
use tokio::runtime::current_thread::Handle;
use tokio::runtime::Runtime;

use super::Core as AppContext;
use super::Context;

type P4RuntimeClient =
    crate::proto::p4runtime::client::P4RuntimeClient<tonic::transport::channel::Channel>;

pub struct ContextDriver<E, T> {
    pub core_request_receiver: UnboundedReceiver<CoreRequest<E>>,
    pub event_receiver: UnboundedReceiver<CoreEvent<E>>,
    pub request_receiver: UnboundedReceiver<NorthboundRequest>,
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
                        _ => {

                        }
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
                            if let Err(e) = handle.master_up(device_id,m).await {
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

    async fn add_device(device: &Device, ctx: &mut AppContext<E>) -> bool {
        let name = &device.name;
        match device.typ {
            DeviceType::MASTER {
                ref socket_addr,
                device_id,
                pipeconf,
            } => {
                if ctx.connections.contains_key(&device.id) {
                    error!(target:"core","Device with name existed: {:?}",device.name);
                    return false;
                }
                let pipeconf_obj = ctx.pipeconf.get(&pipeconf);
                if pipeconf_obj.is_none() {
                    error!(target:"core","pipeconf not found: {:?}",pipeconf);
                    return false;
                }
                let pipeconf = pipeconf_obj.unwrap().clone();
                let bmv2connection = Bmv2SwitchConnection::try_new(
                    name,
                    socket_addr,
                    Bmv2ConnectionOption {
                        p4_device_id: device_id,
                        inner_device_id: Some(device.id.0),
                        ..Default::default()
                    },
                )
                .await;
                if let Err(e) = ctx.add_bmv2_connection(bmv2connection, &pipeconf).await {
                    error!(target:"core","add {} connection fail: {:?}",name,e);
                    ctx.event_sender
                        .send(CoreEvent::Event(
                            CommonEvents::DeviceLost(device.id).into_e(),
                        ))
                        .await;
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
        ctx.connections.remove(&id);
        ctx.event_sender
            .send(CoreEvent::Event(CommonEvents::DeviceLost(id).into_e()))
            .await
            .unwrap();
        true
    }

    pub async fn run_to_end(self) {
        self.run().await;
    }
}
