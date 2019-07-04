use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use byteorder::ByteOrder;

pub struct Value;

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
        self.octets().to_vec()
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