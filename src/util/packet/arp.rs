use crate::util::packet::Packet;
use bytes::{BytesMut, Bytes};
use byteorder::ByteOrder;

pub struct Arp {
    hw_type:u16,
    proto_type:u16,
    hw_addr_len:u8,
    proto_addr_len:u8,
    opcode:u16
}

impl Packet for Arp {
    type Payload = ();

    fn from_bytes(mut b: BytesMut) -> Option<Self> {
        if b.len() < 8 {
            return None;
        }
        let hw_type = bytes::BigEndian::read_u16(b.split_to(2).as_ref());
        let proto_type = bytes::BigEndian::read_u16(b.split_to(2).as_ref());
        let hw_addr_len = b.split_to(1).as_ref()[0];
        let proto_addr_len = b.split_to(1).as_ref()[0];
        let opcode = bytes::BigEndian::read_u16(b.split_to(2).as_ref());
        Some(Arp {
            hw_type,
            proto_type,
            hw_addr_len,
            proto_addr_len,
            opcode
        })
    }

    fn into_bytes(self) -> Bytes {
        unimplemented!()
    }
}