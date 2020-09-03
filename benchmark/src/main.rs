/*
commit:53a9405b6da41f22a133b0cc82b2f4277a93c78e
release
420Mb/s
*/

#![allow(warnings)]
#[macro_use]
extern crate rusty_p4;
#[cfg(not(target_env = "msvc"))]
use jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;
use rusty_p4::p4rt;
use rusty_p4::util::flow::*;
//use rusty_p4::app::extended::{ExampleExtended, P4appBuilder, P4appExtended};
use rusty_p4::util::value::EXACT;
use std::path::Path;
use tokio;
//use rusty_p4::app::linkprobe::{LinkProbeLoader, LinkProbeInterceptor};
//use rusty_p4::app::proxyarp::{ProxyArpLoader, ArpInterceptor};
use rusty_p4::util::packet::arp::ETHERNET_TYPE_ARP;
use rusty_p4::p4rt::pipeconf::Pipeconf;
use rusty_p4::representation::{DeviceID, ConnectPoint, Device};
use rusty_p4::util::{publisher::Handler, flow::Flow};
use std::collections::HashMap;
use rusty_p4::event::{CommonEvents, PacketReceived};
use log::info;
use bytes::Bytes;
use std::sync::atomic::{AtomicUsize, Ordering, AtomicBool};
use std::sync::{RwLock, Arc};
use std::time::{Instant, Duration};
use async_trait::async_trait;
use std::process::exit;
use rusty_p4::app::{App, store::install};
use ipip::ipv4;
use tokio::stream::StreamExt;
use tuple_list::{tuple_list_type, tuple_list};
use p4rt::{bmv2::{Bmv2MasterUpdateOption, Bmv2ConnectionOption, Bmv2Event}, pipeconf::DefaultPipeconf};

pub struct Benchmark {
    bytes: Arc<AtomicUsize>,
    // statistic: StatisticService,
}

#[derive(Clone)]
pub struct NewBenchmark {
    device_manager: rusty_p4::p4rt::bmv2::Bmv2Manager,
    bytes: Arc<AtomicUsize>,
}

#[async_trait]
impl App for NewBenchmark {
    type Dependency = tuple_list_type!(rusty_p4::p4rt::bmv2::Bmv2Manager);

    type Option = ();

    fn init<S>(dependencies: Self::Dependency, store: &mut S, option: Self::Option) -> Self where S: rusty_p4::app::store::AppStore {
        let tuple_list!(device_manager) = dependencies;
        let app = NewBenchmark {
            device_manager: device_manager.clone(),
            bytes: Default::default()
        };
        device_manager.subscribe_packet(app.clone());
        device_manager.subscribe_event(app.clone());
        app
    }

    async fn run(&self) {

    }
}

#[async_trait]
impl Handler<PacketReceived> for NewBenchmark {
    async fn handle(&self, packet: PacketReceived) {
        let from = if let Some(from) = self.device_manager.get_packet_connectpoint(&packet) {
            from
        } else {
            panic!("???");
        };
        let b = packet.packet.len();
        let bytes = self.bytes.fetch_add(b, Ordering::AcqRel);
        if from.port == 1 {
            self.device_manager.send_packet(ConnectPoint {
                device: from.device,
                port: 2,
            }, Bytes::from(packet.packet)).await;
        } else if from.port == 2 {
            self.device_manager.send_packet(ConnectPoint {
                device: from.device,
                port: 1,
            }, Bytes::from(packet.packet)).await;
        }
    }
}

#[async_trait]
impl Handler<Bmv2Event> for NewBenchmark {
    async fn handle(&self, event: Bmv2Event) {
        match event {
            Bmv2Event::DeviceAdded(device) => {
                let mut device = self.device_manager.get_device(device).unwrap();
                device.insert_flow(
                    flow!{
                        pipe: "MyIngress",
                        table: "acl" {
                            "hdr.ethernet.etherType" => rusty_p4::util::packet::arp::ETHERNET_TYPE_ARP,
                        }
                        action: "send_to_cpu" {}
                        priority: 1
                    }
                ).await;
                device.insert_flow(
                    flow!{
                        pipe: "MyIngress",
                        table: "acl" {
                            "hdr.ethernet.etherType" => 1u16 /*ICMP*/,
                        }
                        action: "send_to_cpu" {}
                        priority: 1
                    }
                ).await;
            }
        }
    }
}

#[tokio::main]
pub async fn main() {
    flexi_logger::Logger::with_str("info").start().unwrap();

    let pipeconf = DefaultPipeconf::new(
        "benchmark",
        "/home/skye/rusty-p4/benchmark/build/benchmark.p4.p4info.bin",
        "/home/skye/rusty-p4/benchmark/build/benchmark.json",
    );

    let mut app_store = rusty_p4::app::store::DefaultAppStore::default();
    let device_manager: Arc<rusty_p4::p4rt::bmv2::Bmv2Manager> = install(&mut app_store, ());
    let benchmark: Arc<NewBenchmark> = install(&mut app_store, ());

    device_manager.add_device("s1", "172.17.0.2:50051", Bmv2ConnectionOption {
        p4_device_id: 1,
        inner_device_id: Some(1),
        master_update: Some(Bmv2MasterUpdateOption {
            election_id_high: 0,
            election_id_low: 1,
        })
    }, pipeconf).await;

    futures::future::join_all(vec![device_manager.run(), benchmark.run()]).await;
}