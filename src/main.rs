#![allow(warnings)]

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

mod p4rt;
mod util;
mod proto;
mod app;
mod context;
mod error;
mod event;
mod representation;

fn main() {
    let p4info_helper = p4rt::helper::P4InfoHelper::new(&Path::new("/home/skye/rusty-p4/p4test/build/simple.p4.p4info.bin"));
    let bmv2_file = "/home/skye/rusty-p4/p4test/build/simple.json";
    let mut s1 = Bmv2SwitchConnection::new("s1","127.0.0.1:50051",0);
    let mut s2 = Bmv2SwitchConnection::new("s2","127.0.0.1:50052",1);

    let (mut context,mut runtime) = Context::try_new(p4info_helper, bmv2_file.to_owned(), Example {
        counter: 0
    }).unwrap();

    context.add_connection(s1).unwrap();
    context.add_connection(s2).unwrap();

    runtime.run();
}