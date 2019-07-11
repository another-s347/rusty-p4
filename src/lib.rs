#![allow(warnings)]
#![feature(option_flattening)]

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
use crate::app::extended::{extend, ExampleExtended};
use crate::event::CommonEvents;

pub mod p4rt;
pub mod util;
pub mod proto;
pub mod app;
pub mod context;
pub mod error;
pub mod event;
pub mod representation;

#[test]
fn test() {
    flexi_logger::Logger::with_str("trace").start().unwrap();

    let p4info_helper = p4rt::helper::P4InfoHelper::new(&Path::new("/home/skye/rusty-p4/p4test/build/simple.p4.p4info.bin"));
    let bmv2_file = "/home/skye/rusty-p4/p4test/build/simple.json";
    let mut s1 = Bmv2SwitchConnection::new("s1","127.0.0.1:50051",0);
    let mut s2 = Bmv2SwitchConnection::new("s2","127.0.0.1:50052",1);

    let (mut context,mut runtime) = Context::try_new(p4info_helper, bmv2_file.to_owned(), extend(ExampleExtended {

    })).unwrap();

    context.add_connection(s1).unwrap();
    context.add_connection(s2).unwrap();

    runtime.run();
}