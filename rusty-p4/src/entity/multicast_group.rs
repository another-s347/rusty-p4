use crate::entity::{ProtoEntity, ToEntity};
use crate::p4rt::pipeconf::Pipeconf;

#[derive(Clone)]
pub struct MulticastGroupEntry {
    pub multicast_group_id: u32,
    pub replicas: ::std::vec::Vec<Replica>,
}

impl MulticastGroupEntry {
    fn into_proto(mut self) -> crate::proto::p4runtime::MulticastGroupEntry {
        crate::proto::p4runtime::MulticastGroupEntry {
            multicast_group_id: self.multicast_group_id,
            replicas: self.replicas.drain(0..).map(|r| r.into_proto()).collect(),
        }
    }
}

#[derive(Clone)]
pub struct Replica {
    pub egress_port: u32,
    pub instance: u32,
}

impl Replica {
    fn into_proto(self) -> crate::proto::p4runtime::Replica {
        crate::proto::p4runtime::Replica {
            egress_port: self.egress_port,
            instance: self.instance,
        }
    }
}

impl ToEntity for MulticastGroupEntry {
    fn to_proto_entity(&self, pipeconf: &Pipeconf) -> Option<ProtoEntity> {
        Some(ProtoEntity {
            entity: Some(crate::proto::p4runtime::entity::Entity::PacketReplicationEngineEntry(crate::proto::p4runtime::PacketReplicationEngineEntry {
                r#type:Some(crate::proto::p4runtime::packet_replication_engine_entry::Type::MulticastGroupEntry(self.clone().into_proto()))
            }))
        })
    }
}
