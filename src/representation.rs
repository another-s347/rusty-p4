use std::collections::HashSet;
use std::net::IpAddr;
use crate::util::value::MAC;

pub struct Device {
    pub name: String,
    pub ports: HashSet<Port>,
    pub typ: DeviceType,
    pub device_id: u64,
    pub index: usize
}

pub enum DeviceType {
    MASTER {
        management_address:String
    },
    VIRTUAL
}

#[derive(Eq,Hash)]
pub struct Port {
    pub number: u32,
    pub interface: Option<Interface>
}

impl PartialEq for Port {
    fn eq(&self, other: &Self) -> bool {
        self.number==other.number && self.interface==other.interface
    }
}

#[derive(Eq,Hash)]
pub struct Interface {
    pub name: String,
    pub ip: Option<IpAddr>,
    pub mac: Option<MAC>
}

impl PartialEq for Interface {
    fn eq(&self, other: &Self) -> bool {
        self.mac==other.mac && self.ip==other.ip && self.name==other.name
    }
}

pub struct ConnectPoint {
    device: String,
    port: u32
}

