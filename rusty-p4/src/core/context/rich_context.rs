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
use crate::util::flow::{Flow, FlowMatch, FlowTable};
use byteorder::BigEndian;
use byteorder::ByteOrder;
use bytes::Bytes;
use failure::ResultExt;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, Sender};
use futures::future::FutureExt;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use log::{debug, error, info, trace, warn};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use crate::core::connection::bmv2::Bmv2Connection;
use crate::core::connection::ConnectionBox;
use async_trait::async_trait;
use crate::core::context::Context;
use crate::core::DefaultContext;

#[derive(Clone)]
pub struct RichContext<E> {
    pub default_context: DefaultContext<E>,
    pub state: RichContextState
}

#[derive(Default, Clone)]
pub struct RichContextState {
    pub flows: Arc<dashmap::DashMap<(DeviceID, FlowTable), Flow>>
}

impl<E> RichContext<E>
    where
        E: Debug+Event,
{
    pub fn new(
        core_request_sender: futures::channel::mpsc::Sender<CoreRequest>,
        event_sender: futures::channel::mpsc::Sender<CoreEvent<E>>,
        connections: HashMap<DeviceID, ConnectionBox>,
        pipeconf: Arc<HashMap<PipeconfID, Pipeconf>>,
    ) -> RichContext<E> {
        RichContext {
            default_context: DefaultContext::new(core_request_sender, event_sender, connections, pipeconf),
            state: Default::default()
        }
    }
}

#[async_trait]
impl<E> Context<E> for RichContext<E>
    where E:Event
{
    type ContextState = RichContextState;

    fn new(
        core_request_sender: Sender<CoreRequest>,
        event_sender: Sender<CoreEvent<E>>,
        connections: HashMap<DeviceID, ConnectionBox>,
        pipeconf: Arc<HashMap<PipeconfID, Pipeconf>>,
        state: RichContextState
    ) -> Self {
        RichContext {
            default_context: DefaultContext::new(core_request_sender, event_sender, connections, pipeconf),
            state
        }
    }

    fn send_event(&mut self, event: E) {
        self.send_event(event)
    }

    fn get_conn(&self) -> &HashMap<DeviceID, ConnectionBox> {
        self.get_conn()
    }

    fn get_mut_conn(&mut self) -> &mut HashMap<DeviceID, ConnectionBox> {
        self.default_context.get_mut_conn()
    }

    fn get_connectpoint(&self, packet: &PacketReceived) -> Option<ConnectPoint> {
        self.default_context.get_connectpoint(packet)
    }

    async fn insert_flow(&mut self, mut flow: Flow, device: DeviceID) -> Result<Flow, ContextError> {
        let id:(DeviceID, FlowTable) = (device, flow.table.as_ref().clone());
        if self.state.flows.contains_key(&id) {
            self.default_context.set_flow(flow, device, UpdateType::Modify).await
        }
        else {
            self.state.flows.insert(id, flow.clone());
            self.default_context.set_flow(flow, device, UpdateType::Insert).await
        }
    }

    async fn send_packet(&mut self, to: ConnectPoint, packet: Bytes) {
        self.default_context.send_packet(to, packet).await
    }

    fn add_device(&mut self, device: Device) -> bool {
        self.default_context.add_device(device)
    }
}