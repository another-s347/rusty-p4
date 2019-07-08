use super::Packet;
use bytes::{Bytes, BytesMut};
use byteorder::ByteOrder;

pub struct PacketInHeader<P> {
    todo: u16,
    pub payload: P
}

impl<P> Packet for PacketInHeader<P>
    where P:Packet
{
    type Payload = ();

    fn from_bytes(mut b: BytesMut) -> Option<Self> {
        let todo = bytes::BigEndian::read_u16(b.split_to(2).as_ref());
        let payload = P::from_bytes(b);
        if payload.is_none() {
            return None;
        }
        Some(PacketInHeader {
            todo,
            payload: payload.unwrap()
        })
    }

    fn into_bytes(self) -> Bytes {
        unimplemented!()
    }
}