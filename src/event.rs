use crate::proto::p4runtime::PacketIn;
use std::sync::Arc;
use bitfield::fmt::Debug;
use crate::representation::{Device, ConnectPoint, Host, Meter};
use bytes::{BytesMut, Bytes};

pub enum CoreEvent<E> {
    PacketReceived(PacketReceived),
    Event(E),
}

#[derive(Debug, Clone)]
pub struct PacketReceived {
    pub packet: PacketIn,
    pub from: String,
    pub port: u32
}

#[derive(Debug)]
pub enum CoreRequest<E>
{
    AddDevice {
        device: Device,
        reply: Option<()>,
    },
    Event(E),
    PacketOut {
        device: String,
        port: u32,
        packet: Bytes,
    },
    SetMeter(Meter)
}

pub trait Event: Debug + Send + 'static + From<CommonEvents> + Into<CommonEvents> {}

impl Event for CommonEvents {}

#[derive(Clone, Debug)]
pub enum CommonEvents {
    DeviceAdded(Device),
    DeviceUpdate(Device),
    LinkDetected(ConnectPoint,ConnectPoint),
    HostDetected(Host),
    Other {},
}