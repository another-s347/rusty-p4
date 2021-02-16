use crate::service::Service;
use crate::app::P4app;
use crate::app::raw_statistic::read_stratum_load;
use crate::core::DefaultContext;
use crate::event::{Event,CommonEvents};
use async_trait::async_trait;
use std::sync::Arc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::Instant;
use crate::core::connection::ConnectionBox;
use rusty_p4_proto::proto::v1::{CounterEntry, Index};
use std::time::Duration;
use crate::representation::{DeviceID, Interface, Load, StratumLoad};
use crate::entity::counter::CounterIndex;
use crate::core::connection::stratum_bmv2::StratumBmv2Connection;
use tonic::IntoRequest;
use crate::core::context::Context;

type P4RuntimeClient =
crate::proto::p4runtime::p4_runtime_client::P4RuntimeClient<tonic::transport::channel::Channel>;
type GNMIClient = rusty_p4_proto::proto::gnmi::g_nmi_client::GNmiClient<tonic::transport::channel::Channel>;

pub struct StratumStatistic {
    inner_map:InnerMap
}

impl StratumStatistic {
    pub fn new() -> StratumStatistic {
        StratumStatistic {
            inner_map: InnerMap { counter_map: Arc::new(Default::default()) }
        }
    }
}

#[derive(Clone)]
pub(crate) struct InnerMap {
    pub(crate) counter_map: Arc<RwLock<HashMap<(DeviceID,String), StratumLoad>>>,
}

struct StratumCounterTask {
    pub(crate) inner_map:InnerMap,
    pub connection:GNMIClient,
    pub interval:Duration,
    interface:String,
    device:DeviceID
}

impl StratumCounterTask {
    pub async fn run(mut self) {
        loop {
            tokio::time::delay_for(self.interval.clone()).await;
            if let Some(readings) = read_stratum_load(
                &mut self.connection, self.interface.as_str()).await
            {
                self.inner_map.counter_map.write().entry((self.device,self.interface.clone()))
                    .and_modify(|c| c.update(readings))
                    .or_insert({
                        let mut load = StratumLoad::new();
                        load.update(readings);
                        load
                    });
            }
        }
    }
}

#[async_trait]
impl<E, C> P4app<E, C> for StratumStatistic
    where E:Event + Sync, C: Context<E>
{
    async fn on_event(self: &mut Self, event: E, ctx: &mut C) -> Option<E> {
        if let Some(common) = event.try_to_common() {
            match common {
                CommonEvents::DeviceAdded(device) => {
                    if device.typ.is_stratum() {
                        let mut conn = &ctx.get_conn().get(&device.id).unwrap().get_inner::<StratumBmv2Connection>().unwrap().gnmi_client;
                        for p in device.ports.iter() {
                            if let Some(interface) = &p.interface {
                                tokio::spawn(StratumCounterTask {
                                    inner_map: self.inner_map.clone(),
                                    connection: conn.clone(),
                                    interval: Duration::from_secs(5),
                                    interface: interface.name.clone(),
                                    device: device.id
                                }.run());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Some(event)
    }
}

impl Service for StratumStatistic {
    type ServiceType = StratumStatisticService;

    fn get_service(&mut self) -> Self::ServiceType {
        StratumStatisticService {
            inner_map: self.inner_map.clone()
        }
    }
}

#[derive(Clone)]
pub struct StratumStatisticService {
    pub(crate) inner_map:InnerMap,
}

impl StratumStatisticService {
    pub fn get_load_by_counter(&self, interface:String, device:DeviceID) -> Option<StratumLoad> {
        self.inner_map.counter_map.read().get(&(device,interface)).map(|l|l.clone())
    }

    pub fn get_load(&self) -> HashMap<(DeviceID,String),StratumLoad> {
        self.inner_map.counter_map.read().clone()
    }
}

