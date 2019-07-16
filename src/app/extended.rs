use crate::app::common::{CommonOperation, CommonState, MergeResult};
use crate::app::p4App;
use crate::representation::Device;
use crate::context::ContextHandle;
use crate::event::{Event, CommonEvents, PacketReceived};
use serde::export::PhantomData;
use super::linkprobe;
use crate::util::packet::Ethernet;
use bytes::BytesMut;
use crate::util::packet::data::Data;
use crate::util::packet::Packet;

pub trait p4AppExtended<E> {

}

pub struct p4AppExtendedCore<A, E> {
    common:CommonState,
    extension: A,
    phantom:PhantomData<E>
}

impl<A, E> CommonOperation<E> for p4AppExtendedCore<A, E> where E:Event {
    fn merge_device(&mut self, info: Device, ctx:&ContextHandle<E>) -> MergeResult {
        let result = self.common.merge_device(info, ctx);
        result
    }
}

impl<A, E> p4App<E> for p4AppExtendedCore<A, E>
    where A:p4AppExtended<E>, E:Event
{
    fn on_packet(self:&mut Self, packet:PacketReceived, ctx: &ContextHandle<E>) {
        let bytes = BytesMut::from(packet.packet.payload);
        let ethernet:Option<Ethernet<Data>> = Ethernet::from_bytes(bytes);
        if let Some(eth) = ethernet {
            match eth.ether_type {
                0x865 => {

                }
                0x861 => {
                    linkprobe::on_probe_received(packet.from,eth.payload,ctx);
                }
                _=>{
                    dbg!(eth);
                }
            }
        }
    }

    fn on_event(self:&mut Self, event:E, ctx:&ContextHandle<E>) {
        let common:CommonEvents = event.into();
        match common {
            CommonEvents::DeviceAdded(device)=>{
                linkprobe::on_device_added(device,ctx);
            }
            _=>{}
        }
    }
}

pub struct ExampleExtended {

}

impl p4AppExtended<CommonEvents> for ExampleExtended {

}

pub fn extend<E:Event,A:p4AppExtended<E>>(app: A) ->p4AppExtendedCore<A, E>{
    p4AppExtendedCore {
        common: CommonState::new(),
        extension: app,
        phantom: PhantomData
    }
}