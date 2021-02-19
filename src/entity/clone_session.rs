use crate::entity::{ProtoEntity, ToEntity};
use crate::p4rt::pipeconf::DefaultPipeconf;

pub type CloneSession = crate::proto::p4runtime::CloneSessionEntry;
pub type Replica = crate::proto::p4runtime::Replica;

impl ToEntity for CloneSession {
    fn to_proto_entity(&self, pipeconf: &DefaultPipeconf) -> Option<ProtoEntity> {
        Some(ProtoEntity {
            entity: Some(crate::proto::p4runtime::entity::Entity::PacketReplicationEngineEntry(crate::proto::p4runtime::PacketReplicationEngineEntry {
                r#type:Some(crate::proto::p4runtime::packet_replication_engine_entry::Type::CloneSessionEntry(self.clone()))
            }))
        })
    }
}
