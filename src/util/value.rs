use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use byteorder::ByteOrder;
use hex;
use bytes::Bytes;

pub struct Value;

pub struct MACString(pub String);
pub struct MAC([u8;6]);

impl From<Bytes> for MAC {
    fn from(b: Bytes) -> Self {
        let mut s = [0u8;6];
        s.copy_from_slice(b.as_ref());
        MAC(s)
    }
}

impl MAC {
    pub fn of(s:String)-> MAC {
        unimplemented!()
    }
}

impl Value {
    pub fn EXACT<T:Encode>(v:T) -> InnerValue {
        InnerValue::EXACT(v.encode())
    }

    pub fn LPM<T:Encode>(v:T, prefix_len:i32) -> InnerValue {
        InnerValue::LPM(v.encode(), prefix_len)
    }
}

pub enum InnerValue {
    EXACT(Vec<u8>),
    LPM(Vec<u8>, /*prefix_len*/i32),
    TERNARY(Vec<u8>, /*mask*/Vec<u8>),
    RANGE(/*low*/Vec<u8>,/*high*/Vec<u8>)
}

pub struct ParamValue;

impl ParamValue {
    pub fn with(v:Vec<u8>) -> InnerParamValue {
        v
    }

    pub fn of<T:Encode>(v:T) -> InnerParamValue {
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
            IpAddr::V4(ip)=> ip.octets().to_vec(),
            IpAddr::V6(ip)=> ip.octets().to_vec()
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
        let mut vec = vec![0u8;4];
        byteorder::BigEndian::write_u32(&mut vec, *self);
        vec
    }
}

impl Encode for u8 {
    fn encode(&self) -> Vec<u8> {
        vec![*self]
    }
}

impl Encode for MACString {
    fn encode(&self) -> Vec<u8> {
        hex::decode(self.0.replace(':',"")).unwrap()
    }
}