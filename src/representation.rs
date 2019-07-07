use std::collections::HashSet;
use std::net::IpAddr;
use crate::util::value::MAC;

pub struct Device {
    name: String,
    ports: HashSet<Port>,
    typ: DeviceType
}

pub enum DeviceType {
    MASTER,
    VIRTUAL
}

pub struct Port {
    number: u32,
    interface: Option<Interface>
}

pub struct Interface {
    name: String,
    ip: Option<IpAddr>,
    mac: Option<MAC>
}

pub struct ConnectPoint {
    device: String,
    port: u32
}

