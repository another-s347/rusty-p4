use crate::entity::{ProtoEntity, UpdateType};
use crate::proto::p4runtime::PacketIn;
use crate::representation::{ConnectPoint, Device, DeviceID, Host, Link};
use bytes::{Bytes, BytesMut};
use std::fmt::Debug;
use std::sync::Arc;

pub enum CoreEvent<E> {
    PacketReceived(PacketReceived),
    Event(E),
}

#[derive(Debug, Clone)]
pub struct PacketReceived {
    pub packet: PacketIn,
    pub from: ConnectPoint,
}

impl PacketReceived {
    pub fn into_packet_bytes(self) -> Vec<u8> {
        self.packet.payload
    }

    pub fn get_packet_bytes(&self) -> &[u8] {
        &self.packet.payload
    }
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
