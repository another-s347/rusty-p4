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

#[derive(Debug)]
pub enum CoreRequest<E> {
    AddDevice {
        device: Device,
        reply: Option<()>,
    },
    Event(E),
    PacketOut {
        connect_point: ConnectPoint,
        packet: Bytes,
    },
    SetEntity {
        device: DeviceID,
        entity: ProtoEntity,
        op: UpdateType,
    },
}

pub trait Event: Clone + Debug + Send + 'static + From<CommonEvents> + Into<CommonEvents> {}

impl Event for CommonEvents {}

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
