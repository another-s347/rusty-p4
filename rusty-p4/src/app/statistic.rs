use crate::service::Service;
use crate::app::P4app;
use crate::app::raw_statistic::read_counter;
use crate::core::Context;
use crate::event::{Event,CommonEvents};
use async_trait::async_trait;
use std::sync::Arc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::Instant;
use crate::core::connection::ConnectionBox;
use rusty_p4_proto::proto::v1::{CounterEntry, Index};
use std::time::Duration;
use crate::representation::{DeviceID, Interface, Load};
use crate::entity::counter::CounterIndex;
use crate::core::connection::stratum_bmv2::StratumBmv2Connection;

type P4RuntimeClient =
crate::proto::p4runtime::p4_runtime_client::P4RuntimeClient<tonic::transport::channel::Channel>;

pub struct Statistic {
    inner_map:InnerMap
}

impl Statistic {
    pub fn new() -> Statistic {
        Statistic {
            inner_map: InnerMap { counter_map: Arc::new(Default::default()) }
        }
    }
}

#[derive(Clone)]
pub(crate) struct InnerMap {
    pub(crate) counter_map: Arc<RwLock<HashMap<CounterIndex, Load>>>
}

struct CounterTask {
    pub(crate) inner_map:InnerMap,
    pub connection:P4RuntimeClient,
    pub counter_index:CounterIndex,
    pub interval:Duration
}

impl CounterTask {
    pub async fn run(mut self) {
        loop {
            tokio::time::delay_for(self.interval.clone()).await;
            if let Some(readings) = read_counter(
                self.counter_index.to_counter_entry(),
                &mut self.connection).await
            {
                if self.counter_index.index.is_none() {
                    let mut map = self.inner_map.counter_map.write();
                    for entry in readings {
                        let reading = entry.data.as_ref().unwrap();
                        if reading.byte_count==0 && reading.packet_count==0 {
                            continue;
                        }
                        map.entry(CounterIndex::from_counter_entry(&entry,self.counter_index.device))
                            .and_modify(|c|c.update(reading.packet_count,reading.byte_count))
                            .or_insert({
                                let mut load = Load::new();
                                load.update(reading.packet_count, reading.byte_count);
                                load
                            });
                    }
                }
                else {
                    let reading = readings.first().unwrap().data.as_ref().unwrap();
                    self.inner_map.counter_map.write().entry(self.counter_index)
                        .and_modify(|c|c.update(reading.packet_count,reading.byte_count))
                        .or_insert({
                            let mut load = Load::new();
                            load.update(reading.packet_count, reading.byte_count);
                            load
                        });
                }
            }
        }
    }
}

#[async_trait]
impl<E> P4app<E> for Statistic
    where E:Event
{
    async fn on_event(self: &mut Self, event: E, ctx: &mut Context<E>) -> Option<E> {
        if let Some(common) = event.try_to_common() {
            match common {
                CommonEvents::DeviceAdded(device) => {
                    let conn = ctx.connections.get(&device.id).unwrap().p4runtime_client.clone();
                    tokio::spawn(CounterTask {
                        inner_map: self.inner_map.clone(),
                        connection: conn,
                        counter_index: CounterIndex { device: device.id, id: 0, index: None },
                        interval: Duration::from_secs(5)
                    }.run());
                }
                _ => {}
            }
        }
        Some(event)
    }
}

impl Service for Statistic {
    type ServiceType = StatisticService;

    fn get_service(&mut self) -> Self::ServiceType {
        StatisticService {
            inner_map: self.inner_map.clone()
        }
    }
}

#[derive(Clone)]
pub struct StatisticService {
    pub(crate) inner_map:InnerMap,
}

impl StatisticService {
    pub fn get_load_by_counter(&self, counter_index:CounterIndex) -> Option<Load> {
        self.inner_map.counter_map.read().get(&counter_index).map(|l|l.clone())
    }

//    pub fn get_load_by_interface(&self, device:DeviceID, interface:&Interface) -> Option<Load> {
//        let counter_index = self.translator.get(&device)?(interface)?;
//        self.get_load_by_counter(counter_index)
//    }

    pub fn get_load(&self) -> HashMap<CounterIndex,Load> {
        self.inner_map.counter_map.read().clone()
    }
}

