use crate::entity::{ProtoEntity, UpdateType};
use crate::p4rt::pipeconf::{DefaultPipeconf, PipeconfID};
use crate::proto::p4runtime::PacketIn;
use crate::representation::DeviceID;
use bytes::{Bytes, BytesMut};
use rusty_p4_proto::proto::v1::{
    Entity, ForwardingPipelineConfig, MasterArbitrationUpdate, PacketMetadata, Uint128, Update,
};
use std::fmt::Debug;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PacketReceived {
    pub packet: bytes::Bytes,
    pub from: DeviceID,
    pub metadata: Vec<PacketMetadata>,
}
