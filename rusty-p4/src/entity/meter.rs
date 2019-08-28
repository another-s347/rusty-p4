use crate::entity::{ProtoEntity, ToEntity};
use crate::p4rt::pipeconf::Pipeconf;
use crate::p4rt::pure::get_meter_id;
use rusty_p4_proto::proto::v1::{Index, MeterConfig, MeterEntry};

#[derive(Clone, Debug)]
pub struct Meter {
    pub name: &'static str,
    pub index: i64,
    pub cburst: i64,
    pub cir: i64,
    pub pburst: i64,
    pub pir: i64,
}

impl ToEntity for Meter {
    fn to_proto_entity(&self, pipeconf: &Pipeconf) -> Option<ProtoEntity> {
        Some(ProtoEntity {
            entity: Some(crate::proto::p4runtime::entity::Entity::MeterEntry(
                MeterEntry {
                    meter_id: get_meter_id(pipeconf.get_p4info(), self.name).unwrap(),
                    index: Some(Index { index: self.index }),
                    config: Some(MeterConfig {
                        cir: self.cir,
                        cburst: self.cburst,
                        pir: self.pir,
                        pburst: self.pburst,
                    }),
                },
            )),
        })
    }
}
