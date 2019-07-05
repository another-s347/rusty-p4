#![allow()]

use std::path::Path;
use crate::p4runtime::bmv2::Bmv2SwitchConnection;
use futures::stream::Stream;
use futures::future::Future;
use futures03::stream::StreamExt;
use futures03::sink::SinkExt;
use futures::sink::Sink;
use crate::proto::p4runtime::StreamMessageResponse;
use crate::context::Context;
use crate::app::Example;

mod p4runtime;
mod util;
mod proto;
mod app;
mod context;
mod error;

fn main() {
    let p4info_helper = p4runtime::helper::P4InfoHelper::new(&Path::new("/home/skye/rusty-p4/p4test/build/simple.p4.p4info.bin"));
    let bmv2_file = "/home/skye/rusty-p4/p4test/build/simple.json";
    let mut s1 = Bmv2SwitchConnection::new("s1","127.0.0.1:50051",0);

    let (context,mut runtime) = Context::try_new(s1, p4info_helper, &Path::new(bmv2_file), Example {}).unwrap();

    runtime.run();
}