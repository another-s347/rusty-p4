use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use byteorder::{BigEndian, ByteOrder};
use bytes::{Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::fmt::Formatter;

pub struct Value;
pub type MAC = ipip::MAC;

fn vec_to_mac(vec: Vec<u8>) -> [u8; 6] {
    let mut mac = [0u8; 6];
    mac.copy_from_slice(&vec);
    mac
}

pub fn EXACT<T: Encode>(v: T) -> InnerValue {
    InnerValue::EXACT(v.encode())
}

pub fn LPM<T: Encode>(v: T, prefix_len: i32) -> InnerValue {
    InnerValue::LPM(v.encode(), prefix_len)
}

pub fn TERNARY<T: Encode, P: Encode>(v: T, mask: P) -> InnerValue {
    InnerValue::TERNARY(v.encode(), mask.encode())
}

pub fn RANGE<T: Encode, P: Encode>(v: T, p: P) -> InnerValue {
    InnerValue::RANGE(v.encode(), p.encode())
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum InnerValue {
    EXACT(Bytes),
    LPM(Bytes, /*prefix_len*/ i32),
    TERNARY(Bytes, /*mask*/ Bytes),
    RANGE(/*low*/ Bytes, /*high*/ Bytes),
}

pub fn encode<T: Encode>(v: T) -> InnerParamValue {
    v.encode()
}

pub type InnerParamValue = Bytes;

pub trait Encode: Copy {
    fn encode(self) -> Bytes;
}

impl Encode for Ipv4Addr {
    fn encode(self) -> Bytes {
        Bytes::copy_from_slice(self.octets().as_ref())
    }
}

impl Encode for Ipv6Addr {
    fn encode(self) -> Bytes {
        Bytes::copy_from_slice(self.octets().as_ref())
    }
}

impl Encode for IpAddr {
    fn encode(self) -> Bytes {
        match self {
            IpAddr::V4(ip) => Bytes::copy_from_slice(ip.octets().as_ref()),
            IpAddr::V6(ip) => Bytes::copy_from_slice(ip.octets().as_ref()),
        }
    }
}

impl Encode for &str {
    fn encode(self) -> Bytes {
        Bytes::copy_from_slice(self.as_bytes())
    }
}

impl Encode for u32 {
    fn encode(self) -> Bytes {
        Bytes::copy_from_slice(self.to_be_bytes().as_ref())
    }
}

impl Encode for u8 {
    fn encode(self) -> Bytes {
        Bytes::copy_from_slice(self.to_be_bytes().as_ref())
    }
}

impl Encode for i32 {
    fn encode(self) -> Bytes {
        Bytes::copy_from_slice(self.to_be_bytes().as_ref())
    }
}

impl Encode for u16 {
    fn encode(self) -> Bytes {
        Bytes::copy_from_slice(self.to_be_bytes().as_ref())
    }
}

impl Encode for MAC {
    fn encode(self) -> Bytes {
        Bytes::copy_from_slice(self.0.as_ref())
    }
}
