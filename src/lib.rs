#![allow(warnings)]
#![feature(option_flattening)]
#![feature(async_await)]

#[macro_use]
extern crate bitfield;
#[macro_use]
extern crate serde_json;
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
use crate::app::extended::{extend, ExampleExtended, P4appBuilder};
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
<<<<<<< Updated upstream
use crate::app::linkprobe::LinkProbeLoader;

#[tokio::main]
#[test]
pub async fn main() {
    flexi_logger::Logger::with_str("debug").start().unwrap();

    let p4info_helper = p4rt::helper::P4InfoHelper::new(&Path::new("/home/skye/rusty-p4/p4test/build/simple.p4.p4info.bin"));
    let bmv2_file = "/home/skye/rusty-p4/p4test/build/simple.json";

    let app = P4appBuilder::new(ExampleExtended {

    }).with(LinkProbeLoader::new()).build();

    let mut context = Context::try_new(p4info_helper, bmv2_file.to_owned(), app).await.unwrap();

//    context.get_handle().add_device("s1".to_string(),"1".to_string(),1);
}
=======
use crate::app::graph::GraphBase;
use crate::proto::p4data::P4Data_oneof_data::bool;

//#[tokio::main]
//#[test]
//pub async fn main() {
//    flexi_logger::Logger::with_str("debug").start().unwrap();
//
//    let p4info_helper = p4rt::helper::P4InfoHelper::new(&Path::new("/home/skye/rusty-p4/p4test/build/simple.p4.p4info.bin"));
//    let bmv2_file = "/home/skye/rusty-p4/p4test/build/simple.json";
//
//    let mut context = Context::try_new(p4info_helper, bmv2_file.to_owned(), extend(ExampleExtended {
//
//    })).await.unwrap();
//}
>>>>>>> Stashed changes
