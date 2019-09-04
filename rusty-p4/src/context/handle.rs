use super::Connection;
use super::Context;
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
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use tokio::runtime::current_thread::Handle;
use tokio::runtime::Runtime;

#[derive(Clone)]
pub struct ContextHandle<E> {
    pub sender: UnboundedSender<CoreRequest<E>>,
    pipeconf: Arc<HashMap<PipeconfID, Pipeconf>>,
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
        pipeconf: Arc<HashMap<PipeconfID, Pipeconf>>,
    ) -> ContextHandle<E> {
        ContextHandle {
            sender,
            pipeconf,
            connections,
            id_to_name,
            removed_id_to_name,
        }
    }

    pub fn insert_flow(&self, mut flow: Flow, device: DeviceID) -> Result<Flow, ContextError> {
        self.set_flow(flow, device, UpdateType::Insert)
    }

    pub fn set_flow(
        &self,
        mut flow: Flow,
        device: DeviceID,
        update: UpdateType,
    ) -> Result<Flow, ContextError> {
        let hash = crate::util::hash(&flow);
        let connections = self.connections.read().unwrap();
        let connection = connections.get(&device).ok_or(ContextError::from(
            ContextErrorKind::DeviceNotConnected { device },
        ))?;
        let table_entry = flow.to_table_entry(&connection.pipeconf, hash);
        let request = new_write_table_entry(connection.device_id, table_entry, update);
        connection
            .send_request_sync(&request)
            .context(ContextErrorKind::ConnectionError)?;
        flow.metadata = hash;
        Ok(flow)
    }

    pub fn add_device(&self, name: String, address: String, device_id: u64, pipeconf: &str) {
        let id = crate::util::hash(&name);
        let pipeconf = crate::util::hash(pipeconf);
        let device = Device {
            id: DeviceID(id),
            name,
            ports: Default::default(),
            typ: DeviceType::MASTER {
                socket_addr: address,
                device_id,
                pipeconf: PipeconfID(pipeconf),
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

    pub fn send_request(&self, request: CoreRequest<E>) {
        self.sender.unbounded_send(request).unwrap();
    }

    pub fn send_packet(&self, to: ConnectPoint, packet: Bytes) {
        self.sender
            .unbounded_send(CoreRequest::PacketOut {
                connect_point: to,
                packet,
            })
            .unwrap();
    }

    pub fn set_entity<T: crate::entity::ToEntity>(
        &self,
        device: DeviceID,
        update_type: UpdateType,
        entity: &T,
    ) -> Result<(), ContextError> {
        let connections = self.connections.read().unwrap();
        let connection = connections.get(&device).ok_or(ContextError::from(
            ContextErrorKind::DeviceNotConnected { device },
        ))?;
        if let Some(entity) = entity.to_proto_entity(&connection.pipeconf) {
            self.sender
                .unbounded_send(CoreRequest::SetEntity {
                    device,
                    entity,
                    op: update_type,
                })
                .unwrap();
            Ok(())
        } else {
            Err(ContextError::from(ContextErrorKind::EntityIsNone))
        }
    }
}
