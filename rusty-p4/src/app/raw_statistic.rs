use crate::event::Event;
use crate::app::P4app;
use crate::service::Service;
use async_trait::async_trait;
use crate::core::Context;
use crate::event::CommonEvents;
use crate::representation::DeviceID;
use rusty_p4_proto::proto::v1::{CounterEntry, entity::Entity};
use futures::stream::Stream;

type P4RuntimeClient =
crate::proto::p4runtime::client::P4RuntimeClient<tonic::transport::channel::Channel>;

pub struct RawStatistic {

}

impl RawStatistic {
    pub async fn read_counter(counter_entry:CounterEntry, client:&mut P4RuntimeClient) -> Option<Vec<CounterEntry>> {
        let mut response = client.read(crate::proto::p4runtime::ReadRequest {
            device_id:1,
            entities:vec![crate::proto::p4runtime::Entity {
                entity:Some(crate::proto::p4runtime::entity::Entity::CounterEntry(
                    counter_entry
                ))
            }]
        }).await.ok()?;
        let stream = response.get_mut();
        let mut ret = vec![];
        while let Some(msg) = stream.message().await.unwrap() {
            for e in msg.entities {
                match e.entity {
                    Some(Entity::CounterEntry(counter)) => {
                        ret.push(counter)
                    }
                    _ => {}
                }
            }
        }
        Some(ret)
    }
}