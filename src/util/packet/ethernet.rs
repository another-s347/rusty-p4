use byteorder::ByteOrder;
use bytes::{BufMut, Bytes, BytesMut};

use crate::util::packet::Packet;
use crate::util::value::MAC;
use std::fmt::Debug;
use std::fmt::Formatter;

pub struct Ethernet<P> {
    pub src: MAC,
    pub dst: MAC,
    pub ether_type: u16,
    pub payload: P,
}

impl<P> Debug for Ethernet<P> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "\nsrc: {:?}\ndst: {:?}\ntype: {:x}",
            self.src, self.dst, self.ether_type
        )
    }
}

impl<P> Packet for Ethernet<P>
where
    P: Packet,
{
    type Payload = P;

    fn from_bytes(mut b: BytesMut) -> Option<Self> {
        if b.len() < 14 {
            return None;
        }
        let dst: MAC = b.split_to(6).into();
        let src: MAC = b.split_to(6).into();
        let ether_type = bytes::BigEndian::read_u16(b.split_to(2).as_ref());
        let payload = P::from_bytes(b);
        if payload.is_none() {
            return None;
        }
        Some(Ethernet {
            src,
            dst,
            ether_type,
            payload: payload.unwrap(),
        })
    }

    fn into_bytes(self) -> Bytes {
        let mut buf = BytesMut::new();
        let payload = self.payload.into_bytes();
        buf.reserve(14 + payload.len());
        buf.put_slice(&self.dst.0);
        buf.put_slice(&self.src.0);
        buf.put_u16_be(self.ether_type);
        buf.put(payload);
        buf.freeze()
    }
}

impl<P> Ethernet<P> {
    fn is_ether_type(b: &[u8], ether_type: u16) -> bool {
        if b.len() < 14 {
            return false;
        }
        let ahead = (b[12] << 8) as u16 + b[13] as u16;
        if ahead == ether_type {
            true
        } else {
            false
        }
    }
}
