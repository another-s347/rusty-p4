use crate::util::value::MAC;
use bitfield::fmt::Debug;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt::Formatter;
use std::net::{IpAddr, Ipv4Addr};

#[derive(Clone, Debug)]
pub struct Device {
    pub name: String,
    pub ports: HashSet<Port>,
    pub typ: DeviceType,
    pub device_id: u64,
    pub index: usize,
}

#[derive(Eq, Hash, Clone, Debug)]
pub struct Host {
    pub mac: MAC,
    pub ip: Ipv4Addr,
    pub location: ConnectPoint,
}

impl PartialEq for Host {
    fn eq(&self, other: &Self) -> bool {
        self.mac == other.mac && self.ip == other.ip
    }
}

#[derive(Clone, Debug)]
pub enum DeviceType {
    MASTER { socket_addr: String, device_id: u64 },
    VIRTUAL,
}

#[derive(Eq, Hash, Clone, Debug)]
pub struct Port {
    pub number: u32,
    pub interface: Option<Interface>,
}

impl PartialEq for Port {
    fn eq(&self, other: &Self) -> bool {
        self.number == other.number && self.interface == other.interface
    }
}

#[derive(Eq, Hash, Clone, Debug)]
pub struct Interface {
    pub name: String,
    pub ip: Option<IpAddr>,
    pub mac: Option<MAC>,
}

impl PartialEq for Interface {
    fn eq(&self, other: &Self) -> bool {
        self.mac == other.mac && self.ip == other.ip && self.name == other.name
    }
}

#[derive(Eq, Hash, Clone)]
pub struct ConnectPoint {
    pub device: String,
    pub port: u32,
}

impl Debug for ConnectPoint {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "{}[{}]", self.device, self.port)
    }
}

impl PartialEq for ConnectPoint {
    fn eq(&self, other: &Self) -> bool {
        self.device == other.device && self.port == other.port
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectPointRef<'a> {
    pub device: &'a str,
    pub port: u32,
}

impl<'a> ConnectPointRef<'a> {
    pub fn to_owned(&self) -> ConnectPoint {
        ConnectPoint {
            device: self.device.to_owned(),
            port: self.port,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Meter {
    pub device: String,
    pub name: String,
    pub index: i64,
    pub cburst: i64,
    pub cir: i64,
    pub pburst: i64,
    pub pir: i64,
}

#[derive(Clone, Hash, Eq)]
pub struct Link {
    pub one: ConnectPoint,
    pub two: ConnectPoint,
}

impl Debug for Link {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}<->{:?}", self.one, self.two)
    }
}

impl PartialEq for Link {
    fn eq(&self, other: &Self) -> bool {
        (self.one == other.one && self.two == other.two)
            || (self.one == other.two && self.two == other.one)
    }
}
