use crate::proto::p4runtime::PacketIn;
use std::sync::Arc;

pub enum CoreEvent<E> {
    PacketReceived(PacketReceived),
    Event(E)
}

#[derive(Debug, Clone)]
pub struct PacketReceived {
    pub packet: PacketIn,
    pub from: String
}

pub enum CoreRequest<E> {
    AddDevice {
        name: String,
        address: String,
        device_id: u64,
        reply: Option<()>
    },
    Event(E),
    PacketOut {
        device: String,
        port: u32,
        packet: Vec<u8>
    }
}

pub trait Event {
    fn to_common(&self)->CommonEvents;

    fn from_common(e:CommonEvents)->Self;
}

impl Event for CommonEvents {
    fn to_common(&self) -> CommonEvents {
        self.clone()
    }

    fn from_common(e: CommonEvents) -> Self {
        e
    }
}

#[derive(Clone)]
pub enum CommonEvents {
    DeviceAdded(String),
    DeviceUpdate(),
    Other {

    }
}