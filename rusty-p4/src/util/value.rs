use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use byteorder::{BigEndian, ByteOrder};
use bytes::{Bytes, BytesMut};
use hex;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::fmt::Formatter;

pub struct Value;

pub struct MACString(pub String);
#[derive(Eq, Hash, PartialEq, Clone, Copy, Serialize, Deserialize)]
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

    pub fn zero() -> MAC {
        MAC([0x00; 6])
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

pub fn EXACT<T: Encode>(v: T) -> InnerValue {
    InnerValue::EXACT(v.encode())
}

pub fn LPM<T: Encode>(v: T, prefix_len: i32) -> InnerValue {
    InnerValue::LPM(v.encode(), prefix_len)
}

pub fn TERNARY<T: Encode, P: Encode>(v: T, mask: P) -> InnerValue {
    InnerValue::TERNARY(v.encode(), mask.encode())
}

pub fn RANGE<T:Encode,P:Encode>(v:T,p:P)->InnerValue {
    InnerValue::RANGE(v.encode(),p.encode())
}

#[derive(Clone, Debug, Hash)]
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
        Bytes::from(self.octets().as_ref())
    }
}

impl Encode for Ipv6Addr {
    fn encode(self) -> Bytes {
        Bytes::from(self.octets().as_ref())
    }
}

impl Encode for IpAddr {
    fn encode(self) -> Bytes {
        match self {
            IpAddr::V4(ip) => Bytes::from(ip.octets().as_ref()),
            IpAddr::V6(ip) => Bytes::from(ip.octets().as_ref()),
        }
    }
}

impl Encode for &str {
    fn encode(self) -> Bytes {
        Bytes::from(self)
    }
}

impl Encode for u32 {
    fn encode(self) -> Bytes {
        Bytes::from(self.to_be_bytes().as_ref())
    }
}

impl Encode for u8 {
    fn encode(self) -> Bytes {
        Bytes::from(self.to_be_bytes().as_ref())
    }
}

impl Encode for i32 {
    fn encode(self) -> Bytes {
        Bytes::from(self.to_be_bytes().as_ref())
    }
}

impl Encode for u16 {
    fn encode(self) -> Bytes {
        Bytes::from(self.to_be_bytes().as_ref())
    }
}

impl Encode for MAC {
    fn encode(self) -> Bytes {
        //        Bytes::f
        Bytes::from(self.0.as_ref())
    }
}
