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

#[derive(Debug, Clone)]
pub struct PacketReceived {
    pub packet: bytes::Bytes,
    pub from: DeviceID,
    pub metadata: Vec<PacketMetadata>
}