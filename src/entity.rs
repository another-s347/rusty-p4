use crate::p4rt::pipeconf::DefaultPipeconf;

pub mod clone_session;
pub mod meter;
pub mod multicast_group;
pub mod counter;
//pub mod direct_counter;
pub type ProtoEntity = crate::proto::p4runtime::Entity;

#[derive(Debug)]
pub enum UpdateType {
    Insert,
    Modify,
    Delete,
    Unspecified,
}

impl Into<crate::proto::p4runtime::update::Type> for UpdateType {
    fn into(self) -> crate::proto::p4runtime::update::Type {
        match self {
            UpdateType::Insert => crate::proto::p4runtime::update::Type::Insert,
            UpdateType::Modify => crate::proto::p4runtime::update::Type::Modify,
            UpdateType::Delete => crate::proto::p4runtime::update::Type::Delete,
            UpdateType::Unspecified => crate::proto::p4runtime::update::Type::Unspecified,
        }
    }
}

pub trait ToEntity {
    fn to_proto_entity(&self, pipeconf: &DefaultPipeconf) -> Option<ProtoEntity>;
}
