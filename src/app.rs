use crate::proto::p4runtime::PacketIn;
use crate::context::ContextHandle;
use futures03::future::Future;
use crate::event::PacketReceived;
use bytes::Bytes;
use crate::util::packet::packet_in_header::PacketInHeader;
use crate::util::packet::Ethernet;
use crate::util::packet::data::Data;
use crate::util::packet::Packet;

mod netconfig;
mod extended;

pub trait p4App {
    fn on_start(self:&mut Self, ctx:&ContextHandle) {}

    fn on_packet(self:&mut Self, packet:PacketReceived, ctx: &ContextHandle) {}
}

pub struct Example {
    pub counter:u32
}

impl p4App for Example {
    fn on_packet(self:&mut Self, packet:PacketReceived, ctx: &ContextHandle) {
        let packet = Bytes::from(packet.packet.payload);
        let parsed:Option<Ethernet<Data>> = Ethernet::from_bytes(packet);
        if let Some(ethernet) = parsed {
            self.counter+=1;
            println!("Counter == {}, ethernet type == {:x}", self.counter, ethernet.ether_type);
        }
        else {
            println!("packet parse fail");
        }
    }
}