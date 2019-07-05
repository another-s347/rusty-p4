#![feature(existential_type)]

use crate::proto::p4runtime::PacketIn;
use crate::context::ContextHandle;
use futures03::future::Future;


pub trait p4App {
    fn on_start(self:&mut Self, ctx:&ContextHandle) {}

    fn on_packet(self:&mut Self, packet:PacketIn, ctx: &ContextHandle) {
        dbg!(packet);
    }
}

pub struct Example {

}

impl p4App for Example {

}