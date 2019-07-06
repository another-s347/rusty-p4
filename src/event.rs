use crate::proto::p4runtime::PacketIn;

#[derive(Debug, Clone)]
pub struct PacketReceived {
    pub packet: PacketIn,
    pub from: String
}