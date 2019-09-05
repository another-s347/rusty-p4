use crate::util::packet::{Packet, PacketRef};
use crate::util::value::MAC;
use byteorder::ByteOrder;
use bytes::{BufMut, Bytes, BytesMut};
use nom::bytes::complete::take;
use std::net::Ipv4Addr;

pub const ETHERNET_TYPE_ARP: u16 = 0x806;

#[derive(Debug, Clone)]
pub struct Arp {
    pub hw_type: u16,
    pub proto_type: u16,
    pub hw_addr_len: u8,
    pub proto_addr_len: u8,
    pub opcode: ArpOp, //8
    pub sender_mac: MAC,
    pub sender_ip: Ipv4Addr,
    pub target_mac: MAC,
    pub target_ip: Ipv4Addr,
}

#[derive(Debug, Clone)]
pub struct ArpRef<'a> {
    pub hw_type: u16,
    pub proto_type: u16,
    pub hw_addr_len: u8,
    pub proto_addr_len: u8,
    pub opcode: ArpOp,
    pub sender_mac: &'a [u8],
    pub sender_ip: &'a [u8],
    pub target_mac: &'a [u8],
    pub target_ip: &'a [u8],
}

impl<'a> ArpRef<'a> {
    pub fn get_sender_mac(&self) -> MAC {
        MAC::from_slice(self.sender_mac)
    }

    pub fn get_target_mac(&self) -> MAC {
        MAC::from_slice(self.target_mac)
    }

    pub fn get_sender_ipv4(&self) -> Option<Ipv4Addr> {
        if self.proto_addr_len != 4 {
            return None;
        }
        let mut s = [0u8; 4];
        s.clone_from_slice(self.sender_ip);
        Some(Ipv4Addr::from(s))
    }

    pub fn get_target_ipv4(&self) -> Option<Ipv4Addr> {
        if self.proto_addr_len != 4 {
            return None;
        }
        let mut s = [0u8; 4];
        s.clone_from_slice(self.target_ip);
        Some(Ipv4Addr::from(s))
    }
}

impl<'a> PacketRef<'a> for ArpRef<'a> {
    type Payload = ();

    fn self_bytes_hint(&self) -> usize {
        (6 + 2 * self.hw_addr_len + 2 * self.proto_addr_len) as usize
    }

    fn from_bytes(b: &'a [u8]) -> Option<Self> {
        let (b, hw_type) = nom::number::complete::be_u16::<()>(b).ok()?;
        let (b, proto_type) = nom::number::complete::be_u16::<()>(b).ok()?;
        let (b, hw_addr_len) = nom::number::complete::be_u8::<()>(b).ok()?;
        let (b, proto_addr_len) = nom::number::complete::be_u8::<()>(b).ok()?;
        let (b, opcode) = nom::number::complete::be_u16::<()>(b).ok()?;
        let opcode = ArpOp::from(opcode);
        let (b, sender_mac) = take::<_, _, ()>(hw_addr_len)(b).ok()?;
        let (b, sender_ip) = take::<_, _, ()>(proto_addr_len)(b).ok()?;
        let (b, target_mac) = take::<_, _, ()>(hw_addr_len)(b).ok()?;
        let (b, target_ip) = take::<_, _, ()>(proto_addr_len)(b).ok()?;
        Some(ArpRef {
            hw_type,
            proto_type,
            hw_addr_len,
            proto_addr_len,
            opcode,
            sender_mac,
            sender_ip,
            target_mac,
            target_ip,
        })
    }

    fn write_self_to_buf<T: BufMut>(&self, mut buf: T) {
        buf.put_u16_be(self.hw_type);
        buf.put_u16_be(self.proto_type);
        buf.put_u8(self.hw_addr_len);
        buf.put_u8(self.proto_addr_len);
        buf.put_u16_be(self.opcode.into());
        buf.put_slice(self.sender_mac);
        buf.put_slice(self.sender_ip);
        buf.put_slice(self.target_mac);
        buf.put_slice(self.target_ip);
    }

    fn get_payload(&self) -> Option<&Self::Payload> {
        None
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ArpOp {
    Request,
    Reply,
    Unknown(u16),
}

impl From<u16> for ArpOp {
    fn from(op: u16) -> Self {
        match op {
            0x1 => ArpOp::Request,
            0x2 => ArpOp::Reply,
            other => ArpOp::Unknown(other),
        }
    }
}

impl Into<u16> for ArpOp {
    fn into(self) -> u16 {
        match self {
            ArpOp::Unknown(o) => o,
            ArpOp::Reply => 0x2,
            ArpOp::Request => 0x1,
        }
    }
}

impl Packet for Arp {
    type Payload = ();

    fn bytes_hint(&self) -> usize {
        //        (6 + 2 * self.hw_addr_len + 2 * self.proto_addr_len) as usize
        26
    }

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
            target_ip,
        })
    }

    fn into_bytes(self) -> Bytes {
        let mut buffer = BytesMut::new();
        buffer.put_u16_be(self.hw_type);
        buffer.put_u16_be(self.proto_type);
        buffer.put_u8(self.hw_addr_len);
        buffer.put_u8(self.proto_addr_len);
        buffer.put_u16_be(self.opcode.into());
        buffer.put_slice(&self.sender_mac.0);
        buffer.put_slice(&self.sender_ip.octets());
        buffer.put_slice(&self.target_mac.0);
        buffer.put_slice(&self.target_ip.octets());
        buffer.freeze()
    }
}

fn bytes_to_ipv4(bytes: BytesMut) -> Ipv4Addr {
    let mut src = [0u8; 4];
    for (i, src) in src.iter_mut().enumerate() {
        *src = *bytes.get(i).unwrap();
    }
    src.into()
}
