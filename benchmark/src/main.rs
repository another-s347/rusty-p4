/*
commit:53a9405b6da41f22a133b0cc82b2f4277a93c78e
release
420Mb/s
*/

#![allow(warnings)]
#![feature(option_flattening)]
#![feature(async_await)]
#[macro_use]
extern crate rusty_p4;
use rusty_p4::p4rt;
use rusty_p4::util::flow::*;
use rusty_p4::context::{Context, ContextHandle};
//use rusty_p4::app::extended::{ExampleExtended, P4appBuilder, P4appExtended};
use rusty_p4::util::value::EXACT;
use std::path::Path;
use tokio;
//use rusty_p4::app::linkprobe::{LinkProbeLoader, LinkProbeInterceptor};
//use rusty_p4::app::proxyarp::{ProxyArpLoader, ArpInterceptor};
use rusty_p4::util::packet::arp::ETHERNET_TYPE_ARP;
use rusty_p4::p4rt::pipeconf::Pipeconf;
use rusty_p4::representation::{DeviceID, ConnectPoint};
use rusty_p4::util::flow::Flow;
use std::collections::HashMap;
use rusty_p4::context::ContextConfig;
use rusty_p4::event::{CommonEvents, PacketReceived};
use rusty_p4::app::common::CommonState;
use log::{info};
use bytes::Bytes;
use rusty_p4::app::async_app::{AsyncAppsBuilder, AsyncApp};
use std::sync::atomic::{AtomicUsize, Ordering, AtomicBool};
use std::sync::{RwLock, Arc};
use std::time::{Instant, Duration};
use tokio::timer::Interval;

pub struct Benchmark {
    bytes:Arc<AtomicUsize>,
}

impl AsyncApp<CommonEvents> for Benchmark {
    fn on_start(&self, ctx: &ContextHandle<CommonEvents>) {
        let bytes = self.bytes.clone();
        tokio::spawn(async move {
            let mut interval = Interval::new_interval(Duration::from_secs(1));
            loop {
                interval.next().await;
                let old_value = bytes.swap(0,Ordering::Acquire) as f64;
                println!("transfer {} Mbits/sec", old_value*8.0/(1024.0*1024.0));
            }
        });
    }

    fn on_packet(&self, packet: PacketReceived, ctx: &ContextHandle<CommonEvents>) -> Option<PacketReceived> {
//        println!("transfer");
        let b = packet.packet.payload.len();
        self.bytes.fetch_add(b, Ordering::AcqRel);
        if packet.from.port==1 {
            ctx.send_packet(ConnectPoint {
                device: packet.from.device,
                port: 2
            }, Bytes::from(packet.into_packet_bytes()));
        }
        else if packet.from.port==2 {
            ctx.send_packet(ConnectPoint {
                device: packet.from.device,
                port: 1
            }, Bytes::from(packet.into_packet_bytes()));
        }
        None
    }
}

#[tokio::main]
pub async fn main() {
    flexi_logger::Logger::with_str("info").start().unwrap();

    let pipeconf = Pipeconf::new(
        "benchmark",
        "/home/skye/rusty-p4/benchmark/build/benchmark.p4.p4info.bin",
        "/home/skye/rusty-p4/benchmark/build/benchmark.json",
    );

    let mut pipeconfs = HashMap::new();
    pipeconfs.insert(pipeconf.get_id(),pipeconf);

    let mut app_builder = AsyncAppsBuilder::new();
    app_builder.with(1,"benchmark",Benchmark {
        bytes: Arc::new(AtomicUsize::new(0)),
    });
    let app = app_builder.build();

    let (mut context,driver) = Context::try_new(pipeconfs, app, ContextConfig::default()).await.unwrap();

    context.get_handle().add_device("s1".to_string(),"127.0.0.1:50051".to_string(),1,"benchmark");

    driver.run_to_end().await;
}