#![allow(warnings)]
#![feature(option_flattening)]
#![feature(async_await)]
#[macro_use]
extern crate rusty_p4;
use rusty_p4::p4rt;
use rusty_p4::util::flow::*;
use rusty_p4::context::Context;
use rusty_p4::app::extended::{ExampleExtended, P4appBuilder};
use rusty_p4::restore;
use rusty_p4::util::value::EXACT;
use std::path::Path;
use tokio;
use rusty_p4::app::linkprobe::{LinkProbeLoader, LinkProbeInterceptor};
use rusty_p4::app::proxyarp::{ProxyArpLoader, ArpInterceptor};
use rusty_p4::util::packet::arp::ETHERNET_TYPE_ARP;
use rusty_p4::restore::Restore;
use rusty_p4::p4rt::pipeconf::Pipeconf;
use rusty_p4::representation::DeviceID;
use rusty_p4::util::flow::Flow;
use std::collections::HashMap;
use rusty_p4::context::ContextConfig;

struct SimpleLinkProbeInterceptor {
}

impl LinkProbeInterceptor for SimpleLinkProbeInterceptor {
    fn new_flow(&self, device: DeviceID) -> Flow {
        flow!{
            pipe="IngressPipeImpl";
            table="acl";
            key={
                "hdr.ethernet.ether_type"=>0x861u16
            };
            action=send_to_cpu();
            priority=4000;
        }
    }
}

impl ArpInterceptor for SimpleLinkProbeInterceptor {
    fn new_flow(&self, device: DeviceID) -> Flow {
        flow! {
            pipe="IngressPipeImpl";
            table="acl";
            key={
                "hdr.ethernet.ether_type"=>ETHERNET_TYPE_ARP
            };
            action=send_to_cpu();
            priority=4000;
        }
    }
}

#[tokio::main]
pub async fn main() {
    flexi_logger::Logger::with_str("debug").start().unwrap();

    let pipeconf = Pipeconf::new(
        "simple",
        "/home/skye/rusty-p4/p4test/build/simple.p4.p4info.bin",
        "/home/skye/rusty-p4/p4test/build/simple.json",
    );

    let mut pipeconfs = HashMap::new();
    pipeconfs.insert(pipeconf.get_id(),pipeconf);

    let restore = Restore::new("state.json");

    let app = P4appBuilder::new(ExampleExtended {})
        .with(
            LinkProbeLoader::new()
                .with_interceptor("simple",SimpleLinkProbeInterceptor{})
                .build()
        )
        .with(
            ProxyArpLoader::new()
                .with_interceptor("simple",SimpleLinkProbeInterceptor{})
                .build()
        )
        .build();

    let (mut context,driver) = Context::try_new(pipeconfs, app, Some(restore), ContextConfig::default()).await.unwrap();

    driver.run_to_end().await;
}