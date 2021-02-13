use crate::entity::{ProtoEntity, UpdateType};
use crate::proto::p4runtime::PacketIn;
use crate::representation::{ConnectPoint, Device, DeviceID, Host, Link, DeviceType};
use bytes::{Bytes, BytesMut};
use rusty_p4_proto::proto::v1::{
    Entity, ForwardingPipelineConfig, MasterArbitrationUpdate, Uint128, Update, PacketMetadata
};
use std::fmt::Debug;
use std::sync::Arc;
use crate::p4rt::pipeconf::{DefaultPipeconf, PipeconfID};

/// Event indicates that something happened. It shouldn't change the state of Core and Context
pub enum CoreEvent<E> {
    PacketReceived(PacketReceived),
    Event(E),
    Bmv2MasterUpdate(DeviceID,MasterArbitrationUpdate),
}

#[derive(Debug, Clone)]
pub struct PacketReceived {
    pub packet: bytes::Bytes,
    pub from: DeviceID,
    pub metadata: Vec<PacketMetadata>
}

/// Event indicates that something will happen. It may change the state of Core and Context
#[derive(Debug)]
pub enum CoreRequest {
    AddDevice { device: Device },
    RemoveDevice { device: DeviceID },
    AddPipeconf { pipeconf:DefaultPipeconf },
    UpdatePipeconf { device: DeviceID, pipeconf:PipeconfID },
    RemovePipeconf { pipeconf: PipeconfID },
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
    DevicePipeconfUpdate(DeviceID,PipeconfID),
    DeviceUpdate(Device),
    DeviceLost(DeviceID),
    LinkDetected(Link),
    LinkLost(Link),
    HostDetected(Host),
    HostUpdate(Host),
    HostLost(Host),
    PipeconfAdded(PipeconfID),
    PipeconfRemoved(PipeconfID),
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
