use crate::p4rt::pipeconf::PipeconfID;
use crate::util::value::MAC;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::hash::{Hash, Hasher};
use std::net::{IpAddr, Ipv4Addr};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Device {
    pub id: DeviceID,
    pub name: String,
    pub ports: HashSet<Port>,
    pub typ: DeviceType,
    pub device_id: u64,
    pub index: usize,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeviceID(pub u64);

impl ToString for DeviceID {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl Hash for DeviceID {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.0)
    }
}

#[derive(Copy, Eq, Hash, Clone, Debug)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum DeviceType {
    MASTER {
        socket_addr: String,
        device_id: u64,
        pipeconf: PipeconfID,
    },
    VIRTUAL,
}

impl DeviceType {
    pub fn is_master(&self) -> bool {
        match self {
            DeviceType::MASTER {
                socket_addr,
                device_id,
                pipeconf,
            } => true,
            _ => false,
        }
    }
}

#[derive(Eq, Hash, Clone, Debug, Serialize, Deserialize)]
pub struct Port {
    pub name: String,
    pub number: u32,
    pub interface: Option<Interface>,
}

impl PartialEq for Port {
    fn eq(&self, other: &Self) -> bool {
        self.number == other.number && self.interface == other.interface
    }
}

#[derive(Eq, Hash, Clone, Debug, Serialize, Deserialize)]
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

#[derive(Eq, Hash, Copy, Clone, Serialize, Deserialize)]
pub struct ConnectPoint {
    pub device: DeviceID,
    pub port: u32,
}

impl Debug for ConnectPoint {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}[{}]", self.device, self.port)
    }
}

impl PartialEq for ConnectPoint {
    fn eq(&self, other: &Self) -> bool {
        self.device == other.device && self.port == other.port
    }
}

#[derive(Clone, Debug)]
pub struct Meter {
    pub device: DeviceID,
    pub name: String,
    pub index: i64,
    pub cburst: i64,
    pub cir: i64,
    pub pburst: i64,
    pub pir: i64,
}

#[derive(Copy, Clone, Hash, Eq)]
pub struct Link {
    pub src: ConnectPoint,
    pub dst: ConnectPoint,
}

impl Debug for Link {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}->{:?}", self.src, self.dst)
    }
}

impl PartialEq for Link {
    fn eq(&self, other: &Self) -> bool {
        self.src == other.src && self.dst == other.dst
    }
}

#[derive(Clone, Hash, Debug)]
pub struct MulticastGroup {
    pub device: DeviceID,
    pub id: u32,
    pub replica: Vec<Replica>,
}

#[derive(Clone, Hash, Debug)]
pub struct Replica {
    pub port: u32,
    pub instance: u32,
}
