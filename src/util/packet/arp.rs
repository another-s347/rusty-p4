use crate::util::packet::Packet;
use bytes::{BytesMut, Bytes, BufMut};
use byteorder::ByteOrder;
use crate::util::value::MAC;
use std::net::Ipv4Addr;

pub const ETHERNET_TYPE_ARP:u16 = 0x806;

#[derive(Debug, Clone)]
pub struct Arp {
    pub hw_type:u16,
    pub proto_type:u16,
    pub hw_addr_len:u8,
    pub proto_addr_len:u8,
    pub opcode:ArpOp,
    pub sender_mac:MAC,
    pub sender_ip:Ipv4Addr,
    pub target_mac:MAC,
    pub target_ip:Ipv4Addr
}

#[derive(Clone, Debug)]
pub enum ArpOp {
    Request,
    Reply,
    Unknown(u16)
}

impl From<u16> for ArpOp {
    fn from(op: u16) -> Self {
        match op {
            0x1 => ArpOp::Request,
            0x2 => ArpOp::Reply,
            other=>ArpOp::Unknown(other)
        }
    }
}

impl Into<u16> for ArpOp {
    fn into(self) -> u16 {
        match self {
            ArpOp::Unknown(o)=>o,
            ArpOp::Reply=>0x2,
            ArpOp::Request=>0x1
        }
    }
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
        let opcode = bytes::BigEndian::read_u16(b.split_to(2).as_ref()).into();
        let sender_mac = b.split_to(6).into();
        let sender_ip = bytes_to_ipv4(b.split_to(4));
        let target_mac = b.split_to(6).into();
        let target_ip = bytes_to_ipv4(b.split_to(4));
        Some(Arp {
            hw_type,
            proto_type,
            hw_addr_len,
            proto_addr_len,
            opcode,
            sender_mac,
            sender_ip,
            target_mac,
            target_ip
        })
    }

    fn into_bytes(self) -> Bytes {
        let mut buffer = BytesMut::new();
        buffer.put_u16_be(self.hw_type);
        buffer.put_u16_be(self.proto_type);
        buffer.put_u8(self.hw_addr_len);
        buffer.put_u8(self.proto_addr_len);
        buffer.put_u16_be(self.opcode.into());
        buffer.freeze()
    }
}

fn bytes_to_ipv4(bytes:BytesMut) -> Ipv4Addr {
    let mut src = [0u8; 4];
    for (i, src) in src.iter_mut().enumerate() {
        *src = *bytes.get(i).unwrap();
    }
    src.into()
}