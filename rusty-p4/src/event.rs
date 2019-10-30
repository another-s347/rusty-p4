use crate::entity::{ProtoEntity, UpdateType};
use crate::proto::p4runtime::PacketIn;
use crate::representation::{ConnectPoint, Device, DeviceID, Host, Link, DeviceType};
use bytes::{Bytes, BytesMut};
use rusty_p4_proto::proto::v1::{
    Entity, ForwardingPipelineConfig, MasterArbitrationUpdate, Uint128, Update, PacketMetadata
};
use std::fmt::Debug;
use std::sync::Arc;

pub enum CoreEvent<E> {
    PacketReceived(PacketReceived),
    Event(E),
    Bmv2MasterUpdate(DeviceID,MasterArbitrationUpdate),
}

#[derive(Debug, Clone)]
pub struct PacketReceived {
    pub packet: Vec<u8>,
    pub from: DeviceID,
    pub metadata: Vec<PacketMetadata>
}

#[derive(Debug)]
pub enum CoreRequest<E> {
    AddDevice { device: Device, reply: Option<()> },
    RemoveDevice { device: DeviceID },
    Event(E),
}

pub trait Event: Clone + Debug + Send + 'static {
    fn from_common(c: CommonEvents) -> Self;

    fn try_to_common(&self) -> Option<&CommonEvents>;
}

impl Event for CommonEvents {
    fn from_common(c: CommonEvents) -> Self {
        c
    }

    fn try_to_common(&self) -> Option<&CommonEvents> {
        Some(self)
    }
}

impl CommonEvents {
    pub fn into_e<E>(self) -> E
    where
        E: Event,
    {
        E::from_common(self)
    }
}

#[derive(Clone, Debug)]
pub enum CommonEvents {
    DeviceAdded(Device),
    DeviceMasterUp(DeviceID),
    DeviceUpdate(Device),
    DeviceLost(DeviceID),
    LinkDetected(Link),
    LinkLost(Link),
    HostDetected(Host),
    HostUpdate(Host),
    HostLost(Host),
    Other,
}

pub struct NorthboundRequest {
    pub name: String,
    pub path: String,
    pub args: NorthboundArgType,
    pub payload: Option<Bytes>,
    pub reply: tokio::sync::oneshot::Sender<Result<Bytes, i32>>,
}

pub enum NorthboundArgType {
    Form(String),
    Json(String),
    Binary(Bytes),
}
