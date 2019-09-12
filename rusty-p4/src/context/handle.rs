use super::Connection;
use super::Context;
use crate::app::P4app;
use crate::entity::UpdateType;
use crate::error::{ContextError, ContextErrorKind};
use crate::event::{CommonEvents, CoreEvent, CoreRequest, Event, PacketReceived};
use crate::p4rt::bmv2::Bmv2SwitchConnection;
use crate::p4rt::pipeconf::{Pipeconf, PipeconfID};
use crate::p4rt::pure::{
    new_packet_out_request, new_set_entity_request, new_write_table_entry, table_entry_to_entity,
};
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
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::compat::*;
use futures::future::FutureExt;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
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
    connections: Arc<HashMap<DeviceID, Arc<Connection>>>,
    pipeconf: Arc<HashMap<PipeconfID, Pipeconf>>,
}

impl<E> ContextHandle<E>
where
    E: Debug,
{
    pub fn new(
        sender: UnboundedSender<CoreRequest<E>>,
        connections: Arc<HashMap<DeviceID, Arc<Connection>>>,
        pipeconf: Arc<HashMap<PipeconfID, Pipeconf>>,
    ) -> ContextHandle<E> {
        ContextHandle {
            sender,
            connections,
            pipeconf,
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
        let connection = self.connections.get(&device).ok_or(ContextError::from(
            ContextErrorKind::DeviceNotConnected { device },
        ))?;
        let table_entry = flow.to_table_entry(&connection.pipeconf, hash);
        if let Some(c) = self.connections.get(&device) {
            let request =
                new_set_entity_request(1, table_entry_to_entity(table_entry), update.into());
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

    pub fn add_device_object(&self, device: Device) {
        self.sender
            .unbounded_send(CoreRequest::AddDevice {
                device,
                reply: None,
            })
            .unwrap()
    }

    pub fn add_device_with_pipeconf_id(
        &self,
        name: String,
        address: String,
        device_id: u64,
        pipeconf: PipeconfID,
    ) {
        let id = crate::util::hash(&name);
        let device = Device {
            id: DeviceID(id),
            name,
            ports: Default::default(),
            typ: DeviceType::MASTER {
                socket_addr: address,
                device_id,
                pipeconf,
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
        if let Some(c) = self.connections.get(&to.device) {
            let request = new_packet_out_request(&c.pipeconf, to.port, packet);
            let result = c.send_stream_request(request);
            if result.is_err() {
                error!(target:"context","packet out err {:?}", result.err().unwrap());
            }
        } else {
            // find device name
            error!(target:"context","PacketOut error: connection not found for device {:?}.", to.device);
        }
    }

    pub fn set_entity<T: crate::entity::ToEntity>(
        &self,
        device: DeviceID,
        update_type: UpdateType,
        entity: &T,
    ) -> Result<(), ContextError> {
        let connection = self.connections.get(&device).ok_or(ContextError::from(
            ContextErrorKind::DeviceNotConnected { device },
        ))?;
        if let Some(entity) = entity.to_proto_entity(&connection.pipeconf) {
            if let Some(c) = self.connections.get(&device) {
                let request = new_set_entity_request(1, entity, update_type.into());
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
            Ok(())
        } else {
            Err(ContextError::from(ContextErrorKind::EntityIsNone))
        }
    }

    pub fn remove_device(&self, device: DeviceID) {
        self.sender
            .unbounded_send(CoreRequest::RemoveDevice { device })
            .unwrap();
    }
}
