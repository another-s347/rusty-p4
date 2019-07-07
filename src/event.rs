use crate::proto::p4runtime::PacketIn;
use std::sync::Arc;

pub enum CoreEvent {
    PacketReceived(PacketReceived),
    DeviceAdded(String)
}

#[derive(Debug, Clone)]
pub struct PacketReceived {
    pub packet: PacketIn,
    pub from: String
}