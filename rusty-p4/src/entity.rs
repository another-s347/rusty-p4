use crate::p4rt::pipeconf::Pipeconf;

pub mod clone_session;
pub mod meter;
pub mod multicast_group;
pub type ProtoEntity = crate::proto::p4runtime::Entity;
pub type UpdateType = crate::proto::p4runtime::update::Type;

pub trait ToEntity {
    fn to_proto_entity(&self, pipeconf: &Pipeconf) -> Option<ProtoEntity>;
}
