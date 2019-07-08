use byteorder::ByteOrder;
use bytes::{Bytes, BytesMut};

use crate::util::packet::Packet;
use crate::util::value::MAC;

pub struct Ethernet<P>
    where P:Packet
{
    pub src: MAC,
    pub dst: MAC,
    pub ether_type: u16,
    pub payload: P
}

impl<P> Packet for Ethernet<P>
    where P:Packet
{
    type Payload = P;

    fn from_bytes(mut b: BytesMut) -> Option<Self> {
        if b.len() < 14 {
            return None;
        }
        let dst:MAC = b.split_to(6).into();
        let src:MAC = b.split_to(6).into();
        let ether_type = bytes::BigEndian::read_u16(b.split_to(2).as_ref());
        let payload = P::from_bytes(b);
        if payload.is_none() {
            return None;
        }
        Some(Ethernet {
            src,
            dst,
            ether_type,
            payload: payload.unwrap()
        })
    }

    fn into_bytes(self) -> Bytes {
        unimplemented!()
    }
}