#![allow(warnings)]
#![feature(option_flattening)]
#![feature(async_await)]
#![feature(specialization)]

#[macro_use] extern crate bitfield;
#[macro_use] extern crate serde_json;
#[macro_use] extern crate failure;
use std::path::Path;
use crate::p4rt::bmv2::Bmv2SwitchConnection;
use futures::stream::Stream;
use futures::future::Future;
use futures03::stream::StreamExt;
use futures03::sink::SinkExt;
use futures::sink::Sink;
use crate::proto::p4runtime::StreamMessageResponse;
use crate::context::Context;
use crate::app::Example;
use crate::app::extended::{ExampleExtended, P4appBuilder};
use crate::event::CommonEvents;
use log::{info, trace, warn, debug, error};

pub mod p4rt;
pub mod util;
pub mod proto;
pub mod app;
pub mod context;
pub mod error;
pub mod event;
pub mod representation;
pub mod restore;

use tokio;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use crate::app::linkprobe::LinkProbeLoader;
use crate::restore::Restore;
use crate::p4rt::pipeconf::Pipeconf;
use std::collections::HashMap;

#[tokio::main]
#[test]
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

    let app = P4appBuilder::new(ExampleExtended {

    }).with(LinkProbeLoader::new()).build();

    let mut context = Context::try_new(pipeconfs, app, Some(restore)).await.unwrap();
}
