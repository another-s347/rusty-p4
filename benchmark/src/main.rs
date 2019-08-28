#![allow(warnings)]
#![feature(option_flattening)]
#![feature(async_await)]
#[macro_use]
extern crate rusty_p4;
use rusty_p4::p4rt;
use rusty_p4::util::flow::*;
use rusty_p4::context::{Context, ContextHandle};
use rusty_p4::app::extended::{ExampleExtended, P4appBuilder, P4appExtended};
use rusty_p4::restore;
use rusty_p4::util::value::EXACT;
use std::path::Path;
use tokio;
use rusty_p4::app::linkprobe::{LinkProbeLoader, LinkProbeInterceptor};
use rusty_p4::app::proxyarp::{ProxyArpLoader, ArpInterceptor};
use rusty_p4::util::packet::arp::ETHERNET_TYPE_ARP;
use rusty_p4::restore::Restore;
use rusty_p4::p4rt::pipeconf::Pipeconf;
use rusty_p4::representation::{DeviceID, ConnectPoint};
use rusty_p4::util::flow::Flow;
use std::collections::HashMap;
use rusty_p4::context::ContextConfig;
use rusty_p4::event::{CommonEvents, PacketReceived};
use rusty_p4::app::common::CommonState;
use log::{info};
use bytes::Bytes;

pub struct Benchmark {}

impl P4appExtended<CommonEvents> for Benchmark {
    fn on_packet(
        self: &mut Self,
        packet: PacketReceived,
        ctx: &ContextHandle<CommonEvents>,
        state: &CommonState,
    ) {
        if packet.from.port==1 {
            ctx.send_packet(ConnectPoint {
                device: packet.from.device,
                port: 2
            }, Bytes::from(packet.packet.payload));
        }
        else if packet.from.port==2 {
            ctx.send_packet(ConnectPoint {
                device: packet.from.device,
                port: 1
            }, Bytes::from(packet.packet.payload));
        }
    }
}

#[tokio::main]
pub async fn main() {
    flexi_logger::Logger::with_str("debug").start().unwrap();

    let pipeconf = Pipeconf::new(
        "benchmark",
        "/home/skye/rusty-p4/benchmark/build/benchmark.p4.p4info.bin",
        "/home/skye/rusty-p4/benchmark/build/benchmark.json",
    );

    let mut pipeconfs = HashMap::new();
    pipeconfs.insert(pipeconf.get_id(),pipeconf);

    let app = P4appBuilder::new(Benchmark {}).build();

    let (mut context,driver) = Context::try_new(pipeconfs, app, None, ContextConfig::default()).await.unwrap();

    context.get_handle().add_device("s1".to_string(),"127.0.0.1:50051".to_string(),1,"benchmark");

    driver.run_to_end().await;
}