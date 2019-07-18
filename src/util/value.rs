use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use bitfield::fmt::Debug;
use byteorder::{BigEndian, ByteOrder};
use bytes::{Bytes, BytesMut};
use hex;
use std::fmt::Formatter;

pub struct Value;

pub struct MACString(pub String);
#[derive(Eq, Hash, PartialEq, Clone, Copy)]
pub struct MAC(pub [u8; 6]);

impl From<BytesMut> for MAC {
    fn from(b: BytesMut) -> Self {
        let mut s = [0u8; 6];
        s.copy_from_slice(b.as_ref());
        MAC(s)
    }
}

impl MAC {
    pub fn of(s: &str) -> MAC {
        let vec = hex::decode(s.replace(':', "")).unwrap();
        MAC(vec_to_mac(vec))
    }

    pub fn broadcast() -> MAC {
        MAC([0xff; 6])
    }

    pub fn is_broadcast(&self) -> bool {
        self.0 == [0xff; 6]
    }

    pub fn is_multicast(&self) -> bool {
        self.0[0] == 0x33 && self.0[1] == 0x33
    }
}

impl Debug for MAC {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

fn vec_to_mac(vec: Vec<u8>) -> [u8; 6] {
    let mut mac = [0u8; 6];
    mac.copy_from_slice(&vec);
    mac
}

impl Value {
    pub fn EXACT<T: Encode>(v: T) -> InnerValue {
        InnerValue::EXACT(v.encode())
    }

    pub fn LPM<T: Encode>(v: T, prefix_len: i32) -> InnerValue {
        InnerValue::LPM(v.encode(), prefix_len)
    }

    pub fn TERNARY<T: Encode>(v: T, mask: Vec<u8>) -> InnerValue {
        InnerValue::TERNARY(v.encode(), mask)
    }
}

#[derive(Clone)]
pub enum InnerValue {
    EXACT(Vec<u8>),
    LPM(Vec<u8>, /*prefix_len*/ i32),
    TERNARY(Vec<u8>, /*mask*/ Vec<u8>),
    RANGE(/*low*/ Vec<u8>, /*high*/ Vec<u8>),
}

pub struct ParamValue;

impl ParamValue {
    pub fn with(v: Vec<u8>) -> InnerParamValue {
        v
    }

    pub fn of<T: Encode>(v: T) -> InnerParamValue {
        v.encode()
    }
}

pub type InnerParamValue = Vec<u8>;

pub trait Encode {
    fn encode(&self) -> Vec<u8>;
}

impl Encode for Ipv4Addr {
    fn encode(&self) -> Vec<u8> {
        let b = self.octets();
        b.to_vec()
    }
}

impl Encode for Ipv6Addr {
    fn encode(&self) -> Vec<u8> {
        self.octets().to_vec()
    }
}

impl Encode for IpAddr {
    fn encode(&self) -> Vec<u8> {
        match self {
            IpAddr::V4(ip) => ip.octets().to_vec(),
            IpAddr::V6(ip) => ip.octets().to_vec(),
        }
    }
}

impl Encode for &str {
    fn encode(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl Encode for u32 {
    fn encode(&self) -> Vec<u8> {
        let mut vec = vec![0u8; 4];
        byteorder::BigEndian::write_u32(&mut vec, *self);
        vec
    }
}

impl Encode for u8 {
    fn encode(&self) -> Vec<u8> {
        vec![*self]
    }
}

impl Encode for u16 {
    fn encode(&self) -> Vec<u8> {
        let mut buffer = vec![0u8; 2];
        BigEndian::write_u16(&mut buffer, *self);
        buffer
    }
}

impl Encode for MAC {
    fn encode(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}
