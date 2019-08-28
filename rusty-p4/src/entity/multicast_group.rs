use crate::entity::{ProtoEntity, ToEntity};
use crate::p4rt::pipeconf::Pipeconf;

pub type MulticastGroup = crate::proto::p4runtime::MulticastGroupEntry;
pub type Replica = crate::proto::p4runtime::Replica;

impl ToEntity for MulticastGroup {
    fn to_proto_entity(&self, pipeconf: &Pipeconf) -> Option<ProtoEntity> {
        Some(ProtoEntity {
            entity: Some(crate::proto::p4runtime::entity::Entity::PacketReplicationEngineEntry(crate::proto::p4runtime::PacketReplicationEngineEntry {
                r#type:Some(crate::proto::p4runtime::packet_replication_engine_entry::Type::MulticastGroupEntry(self.clone()))
            }))
        })
    }
}
