use crate::app::P4app;
use crate::entity::UpdateType;
use crate::error::{ContextError, ContextErrorKind};
use crate::event::{CommonEvents, CoreEvent, CoreRequest, Event, PacketReceived};
use crate::p4rt::bmv2::Bmv2SwitchConnection;
use crate::p4rt::pipeconf::{Pipeconf, PipeconfID};
use crate::p4rt::pure::{
    new_packet_out_request, new_set_entity_request, new_write_table_entry, table_entry_to_entity,
};
use crate::proto::p4runtime::{
    stream_message_response, Entity, Index, MeterEntry, PacketIn, StreamMessageRequest,
    StreamMessageResponse, Uint128, Update, WriteRequest, WriteResponse,
};
use rusty_p4_proto::proto::v1::MasterArbitrationUpdate;
use crate::representation::{ConnectPoint, Device, DeviceID, DeviceType};
use crate::util::flow::Flow;
use byteorder::BigEndian;
use byteorder::ByteOrder;
use bytes::Bytes;
use failure::ResultExt;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::future::FutureExt;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use log::{debug, error, info, trace, warn};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use tokio::runtime::current_thread::Handle;
use tokio::runtime::Runtime;
use crate::core::connection::bmv2::Bmv2Connection;
use crate::core::connection::ConnectionBox;

#[derive(Clone)]
pub struct Context<E> {
    pub core_request_sender: UnboundedSender<CoreRequest>,
    event_sender: UnboundedSender<CoreEvent<E>>,
    connections: HashMap<DeviceID, ConnectionBox>,
    pipeconf: Arc<HashMap<PipeconfID, Pipeconf>>,
}

impl<E> Context<E>
where
    E: Debug+Event,
{
    pub fn new(
        core_request_sender: UnboundedSender<CoreRequest>,
        event_sender: UnboundedSender<CoreEvent<E>>,
        connections: HashMap<DeviceID, ConnectionBox>,
        pipeconf: Arc<HashMap<PipeconfID, Pipeconf>>,
    ) -> Context<E> {
        Context {
            core_request_sender,
            event_sender,
            connections,
            pipeconf,
        }
    }

    pub fn update_pipeconf(&mut self, device:DeviceID, pipeconf:PipeconfID) {
        self.core_request_sender.unbounded_send(CoreRequest::UpdatePipeconf {
            device, pipeconf
        }).unwrap();
    }

    pub async fn insert_flow(&mut self, mut flow: Flow, device: DeviceID) -> Result<Flow, ContextError> {
        self.set_flow(flow, device, UpdateType::Insert).await
    }

    pub async fn set_flow(
        &mut self,
        mut flow: Flow,
        device: DeviceID,
        update: UpdateType,
    ) -> Result<Flow, ContextError> {
        let hash = crate::util::hash(&flow);
        let connection = self.connections.get_mut(&device).ok_or(ContextError::from(
            ContextErrorKind::DeviceNotConnected { device },
        ))?;
        let table_entry = flow.to_table_entry(&connection.pipeconf, hash);
        let request =
            new_set_entity_request(1, table_entry_to_entity(table_entry), update.into());
        match connection.p4runtime_client.write(tonic::Request::new(request)).await {
            Ok(response) => {
                debug!(target:"core","set entity response: {:?}",response);
            }
            Err(e) => {
                error!(target:"core","grpc send error: {:?}",e);
            }
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
        self.core_request_sender
            .unbounded_send(CoreRequest::AddDevice {
                device
            })
            .unwrap()
    }

    pub fn add_device_object(&self, device: Device) {
        self.core_request_sender
            .unbounded_send(CoreRequest::AddDevice {
                device,
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
        self.core_request_sender
            .unbounded_send(CoreRequest::AddDevice {
                device,
            })
            .unwrap()
    }

    pub fn send_event(&self, event: E) {
        self.event_sender
            .unbounded_send(CoreEvent::Event(event))
            .unwrap();
    }

    pub fn send_request(&self, request: CoreRequest) {
        self.core_request_sender.unbounded_send(request).unwrap();
    }

    pub async fn send_packet(&mut self, to: ConnectPoint, packet: Bytes) {
        if let Some(c) = self.connections.get_mut(&to.device) {
            let request = new_packet_out_request(&c.pipeconf, to.port, packet);
            if let Err(err) = c.send_stream_request(request).await {
                error!(target:"core","packet out err {:?}", err);
            }
        } else {
            // find device name
            error!(target:"core","PacketOut error: connection not found for device {:?}.", to.device);
        }
    }

    pub async fn set_entity<T: crate::entity::ToEntity>(
        &mut self,
        device: DeviceID,
        update_type: UpdateType,
        entity: &T,
    ) -> Result<(), ContextError> {
        let connection = self.connections.get_mut(&device).ok_or(ContextError::from(
            ContextErrorKind::DeviceNotConnected { device },
        ))?;
        if let Some(entity) = entity.to_proto_entity(&connection.pipeconf) {
            let request = new_set_entity_request(1, entity, update_type.into());
            match connection.p4runtime_client.write(tonic::Request::new(request)).await {
                Ok(response) => {
                    debug!(target:"core","set entity response: {:?}",response);
                }
                Err(e) => {
                    error!(target:"core","grpc send error: {:?}",e);
                }
            }
            Ok(())
        } else {
            Err(ContextError::from(ContextErrorKind::EntityIsNone))
        }
    }

    pub fn remove_device(&self, device: DeviceID) {
        self.core_request_sender
            .unbounded_send(CoreRequest::RemoveDevice { device })
            .unwrap();
    }

//    pub async fn master_up(&mut self, device:DeviceID, master_update:MasterArbitrationUpdate)
//        -> Result<(), ContextError>
//    {
//        let mut context = self.clone();
//        let connection = self.connections.get_mut(&device).ok_or(ContextError::from(
//            ContextErrorKind::DeviceNotConnected { device },
//        ))?;
//        connection.master_up(master_update, &mut context).await
//    }

    pub fn try_get_connectpoint(&self, packet:&PacketReceived) -> Option<ConnectPoint> {
        self.connections.get(&packet.from)
            .map(|conn|&conn.pipeconf)
            .and_then(|pipeconf|{
                packet.metadata.iter()
                    .find(|x| x.metadata_id == pipeconf.packetin_ingress_id)
                    .map(|x| BigEndian::read_u16(x.value.as_ref()))
            })
            .map(|port|ConnectPoint {
                device: packet.from,
                port: port as u32
            })
    }
}
