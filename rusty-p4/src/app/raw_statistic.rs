use crate::event::Event;
use crate::app::P4app;
use crate::service::Service;
use async_trait::async_trait;
use crate::core::Context;
use crate::event::CommonEvents;
use crate::representation::{DeviceID, Load, StratumLoad};
use rusty_p4_proto::proto::v1::{CounterEntry, entity::Entity, DirectCounterEntry, TableEntry};
use futures::stream::Stream;
use rusty_p4_proto::proto::gnmi::{GetRequest};
use rusty_p4_proto::proto::gnmi::typed_value::Value;

type P4RuntimeClient =
crate::proto::p4runtime::p4_runtime_client::P4RuntimeClient<tonic::transport::channel::Channel>;
type GNMIClient = rusty_p4_proto::proto::gnmi::g_nmi_client::GNmiClient<tonic::transport::channel::Channel>;

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

pub async fn read_stratum_load(client:&mut GNMIClient, interface:&str) -> Option<[i64;14]> {
    let request = rusty_p4_proto::proto::gnmi::GetRequest {
        prefix:None,
        path:vec![crate::gnmi::new_gnmi_path(&format!("/interfaces/interface[name={}]/state/counters",interface))],
        r#type:1,
        encoding:2,
        use_models:vec![],
        extension:vec![]
    };
    let mut response = client.get(request).await.ok()?;
    let mut loads = [0;14];
    for n in response.get_ref().notification.iter() {
        let name = n.update.first()
            .and_then(|u|u.path.as_ref())
            .and_then(|p|p.elem.last())
            .map(|p|p.name.as_str());
        let value = n.update.first()
            .and_then(|u|u.val.as_ref())
            .and_then(|v|v.value.as_ref())
            .and_then(|v|{
                match v {
                    Value::UintVal(v) => Some(*v),
                    _ => None
                }
            });
        value.iter().zip(name).for_each(|(v,name)|{
            match name {
                "in-broadcast-pkts" => loads[0] = *v as i64,
                "in-discards" => loads[1] = *v as i64,
                "in-errors" => loads[2] = *v as i64,
                "in-fcs-errors" => loads[3] = *v as i64,
                "in-multicast-pkts" => loads[4] = *v as i64,
                "in-octets" => loads[5] = *v as i64,
                "in-unicast-pkts" => loads[6] = *v as i64,
                "in-unknown-protos" => loads[7] = *v as i64,
                "out-broadcast-pkts" => loads[8] = *v as i64,
                "out-discards" => loads[9] = *v as i64,
                "out-errors" => loads[10] = *v as i64,
                "out-multicast-pkts" => loads[11] = *v as i64,
                "out-octets" => loads[12] = *v as i64,
                "out-unicast-pkts" => loads[13] = *v as i64,
                _ => {}
            }
        });
    }
    Some(loads)
}