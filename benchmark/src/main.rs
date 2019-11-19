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
use rusty_p4::core::{Context, Core};
//use rusty_p4::app::extended::{ExampleExtended, P4appBuilder, P4appExtended};
use rusty_p4::util::value::EXACT;
use std::path::Path;
use tokio;
//use rusty_p4::app::linkprobe::{LinkProbeLoader, LinkProbeInterceptor};
//use rusty_p4::app::proxyarp::{ProxyArpLoader, ArpInterceptor};
use rusty_p4::util::packet::arp::ETHERNET_TYPE_ARP;
use rusty_p4::p4rt::pipeconf::Pipeconf;
use rusty_p4::representation::{DeviceID, ConnectPoint, Device};
use rusty_p4::util::flow::Flow;
use std::collections::HashMap;
use rusty_p4::core::core::ContextConfig;
use rusty_p4::event::{CommonEvents, PacketReceived};
use rusty_p4::app::common::CommonState;
use log::info;
use bytes::Bytes;
use std::sync::atomic::{AtomicUsize, Ordering, AtomicBool};
use std::sync::{RwLock, Arc};
use std::time::{Instant, Duration};
use tokio::timer::Interval;
use async_trait::async_trait;
use rusty_p4::app::P4app;
use rusty_p4::app::app_service::AppServiceBuilder;
use rusty_p4::app::raw_statistic::RawStatistic;
use std::process::exit;
use rusty_p4::app::statistic::{Statistic, StatisticService};

pub struct Benchmark {
    bytes: Arc<AtomicUsize>,
    statistic: StatisticService,
}


#[async_trait]
impl P4app<CommonEvents> for Benchmark {
    async fn on_start(&mut self, ctx: &mut Context<CommonEvents>) {
        let bytes = self.bytes.clone();
        let statistic = self.statistic.clone();
        tokio::spawn(async move {
            let mut interval = Interval::new_interval(Duration::from_secs(1));
            loop {
                interval.next().await;
                let old_value = bytes.swap(0, Ordering::Acquire) as f64;
                println!("transfer {} Mbits/sec", old_value * 8.0 / (1024.0 * 1024.0));
                for (index,load) in statistic.get_load() {
                    dbg!(load);
                }
            }
        });
    }

    async fn on_packet(&mut self, packet: PacketReceived, ctx: &mut Context<CommonEvents>) -> Option<PacketReceived> {
        let from = if let Some(from) = ctx.try_get_connectpoint(&packet) {
            from
        } else { return None; };
        let b = packet.packet.len();
        let bytes = self.bytes.fetch_add(b, Ordering::AcqRel);
        if from.port == 1 {
            ctx.send_packet(ConnectPoint {
                device: from.device,
                port: 2,
            }, Bytes::from(packet.packet)).await;
        } else if from.port == 2 {
            ctx.send_packet(ConnectPoint {
                device: from.device,
                port: 1,
            }, Bytes::from(packet.packet)).await;
        }
        None
    }
}

#[tokio::main]
pub async fn main() {
    flexi_logger::Logger::with_str("info").start().unwrap();

    let pipeconf = Pipeconf::new(
        "benchmark",
        "/home/abc/rusty-p4/benchmark/build/benchmark.p4.p4info.bin",
        "/home/abc/rusty-p4/benchmark/build/benchmark.json",
    );

    let mut pipeconfs = HashMap::new();
    pipeconfs.insert(pipeconf.get_id(), pipeconf);

    let mut app_builder = AppServiceBuilder::new();
    let statistic_service = app_builder.with_service(Statistic::new());
    app_builder.with(Benchmark {
        bytes: Arc::new(AtomicUsize::new(0)),
        statistic:statistic_service
    });

    let app = app_builder.build();

    let (mut context, driver) = Core::try_new(pipeconfs, app, ContextConfig::default(), None).await.unwrap();

    context.add_device(Device::new_stratum_bmv2("s1", "127.0.0.1:50001", "benchmark", 1));

    driver.run_to_end().await;
}