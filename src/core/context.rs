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
use nom::lib::std::collections::hash_map::RandomState;

// pub mod default_context;
// pub mod rich_context;

// pub use default_context::DefaultContext;

#[async_trait]
pub trait Context<E>: 'static + Send + Sync + Clone {
    type ContextState: Default + Clone;

    fn new(
        core_request_sender: futures::channel::mpsc::Sender<CoreRequest>,
        event_sender: futures::channel::mpsc::Sender<CoreEvent<E>>,
        connections: HashMap<DeviceID, ConnectionBox>,
        pipeconf: Arc<HashMap<PipeconfID, Pipeconf>>,
        state: Self::ContextState
    ) -> Self;

    fn send_event(&mut self, event: E);

    fn get_conn(&self) -> &HashMap<DeviceID, ConnectionBox>;

    fn get_mut_conn(&mut self) -> &mut HashMap<DeviceID, ConnectionBox>;

    fn get_connectpoint(&self, packet: &PacketReceived) -> Option<ConnectPoint>;

    async fn insert_flow(&mut self, mut flow: Flow, device: DeviceID) -> Result<Flow, ContextError>;

    async fn send_packet(&mut self, to: ConnectPoint, packet: Bytes);

    fn add_device(&mut self, device: Device) -> bool;
}