use super::Packet;
use ipip::MAC;
use bytes::BufMut;
use nom::bytes::complete::take;
use std::net::Ipv4Addr;

pub const ETHERNET_TYPE_ARP: u16 = 0x806;

#[derive(Debug, Clone)]
pub struct Arp<'a> {
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

impl<'a> Arp<'a> {
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

impl<'a> Packet<'a> for Arp<'a> {
    type Payload = ();

    fn self_bytes_hint(&self) -> usize {
        (8 + 2 * self.hw_addr_len + 2 * self.proto_addr_len) as usize
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
        let (_b, target_ip) = take::<_, _, ()>(proto_addr_len)(b).ok()?;
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

    fn write_self_to_buf<T: BufMut>(&self, buf: &mut T) {
        buf.put_u16(self.hw_type);
        buf.put_u16(self.proto_type);
        buf.put_u8(self.hw_addr_len);
        buf.put_u8(self.proto_addr_len);
        buf.put_u16(self.opcode.into());
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

impl From<ArpOp> for u16 {
    fn from(op: ArpOp) -> Self {
        match op {
            ArpOp::Request => 0x1,
            ArpOp::Reply => 0x2,
            ArpOp::Unknown(o) => o
        }
    }
}