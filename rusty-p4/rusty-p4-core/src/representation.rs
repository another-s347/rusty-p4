use crate::{p4rt::pipeconf::PipeconfID, util::publisher::Handler};
// use crate::util::value::MAC;
use serde::{Deserialize, Serialize};
use std::{collections::{HashMap, HashSet}, sync::Arc, sync::Mutex};
use std::fmt::Debug;
use std::fmt::Formatter;
use std::hash::{Hash, Hasher};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Instant;
use async_trait::async_trait;

#[derive(Hash, Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Ord, PartialOrd)]
pub struct DeviceID(pub u64);

impl ToString for DeviceID {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

#[derive(Eq, Hash, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectPoint {
    pub device: DeviceID,
    pub port: u32,
}

impl Debug for ConnectPoint {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}[{}]", self.device, self.port)
    }
}

// #[derive(Clone, Debug)]
// pub struct Load {
//     pub timestamp:Instant,
//     pub packets: i64,
//     pub bytes: i64,
//     pub last_bps: f64,
//     pub last_pps: f64,
//     pub all_bytes: i64,
//     pub all_packets: i64,
//     pub start_timestamp: Instant
// }

// impl Load {
//     pub fn new_with_bytes(b:i64) -> Load {
//         Load {
//             timestamp: Instant::now(),
//             packets: 0,
//             bytes: b,
//             last_bps: 0.0,
//             last_pps: 0.0,
//             all_bytes: b,
//             all_packets: 0,
//             start_timestamp: Instant::now()
//         }
//     }

//     pub fn new_with_packets(p:i64) -> Load {
//         Load {
//             timestamp: Instant::now(),
//             packets: p,
//             bytes: 0,
//             last_bps: 0.0,
//             last_pps: 0.0,
//             all_bytes: 0,
//             all_packets: p,
//             start_timestamp: Instant::now()
//         }
//     }

//     pub fn new() -> Load {
//         Load {
//             timestamp: Instant::now(),
//             packets: 0,
//             bytes: 0,
//             last_bps: 0.0,
//             last_pps: 0.0,
//             all_bytes: 0,
//             all_packets: 0,
//             start_timestamp: Instant::now()
//         }
//     }

//     pub(crate) fn update(&mut self, packet:i64, bytes: i64) {
//         let timestamp = Instant::now();
//         let last = self.timestamp;
//         let dur = timestamp.duration_since(last).as_secs_f64();
//         let diff_packets = packet-self.packets;
//         let diff_bytes = bytes-self.bytes;
//         let last_pps = diff_packets as f64 / dur;
//         let last_bps = diff_bytes as f64 / dur;
//         self.last_bps = last_bps;
//         self.last_pps = last_pps;
//         self.packets = packet;
//         self.bytes = bytes;
//         self.timestamp = timestamp;
//         self.all_bytes+=bytes;
//         self.all_packets+=packet;
//     }

//     pub(crate) fn update_bytes(&mut self, bytes: i64) {
//         let timestamp = Instant::now();
//         let last = self.timestamp;
//         let dur = timestamp.duration_since(last).as_secs_f64();
//         let diff_bytes = bytes-self.bytes;
//         let last_bps = diff_bytes as f64 / dur;
//         self.last_bps = last_bps;
//         self.last_pps = 0.;
//         self.bytes = bytes;
//         self.timestamp = timestamp;
//         self.all_bytes+=bytes;
//     }

//     pub(crate) fn update_packets(&mut self, packet:i64) {
//         let timestamp = Instant::now();
//         let last = self.timestamp;
//         let dur = timestamp.duration_since(last).as_secs_f64();
//         let diff_packets = packet-self.packets;
//         let last_pps = diff_packets as f64 / dur;
//         self.last_bps = 0.;
//         self.last_pps = last_pps;
//         self.packets = packet;
//         self.timestamp = timestamp;
//         self.all_packets+=packet;
//     }
// }

// #[derive(Clone, Debug)]
// pub struct StratumLoad {
//     pub in_broadcast_pkts:Load,
//     pub in_discards:Load,
//     pub in_errors:Load,
//     pub in_fcs_errors:Load,
//     pub in_multicast_pkts:Load,
//     pub in_octets:Load,
//     pub in_unicast_pkts:Load,
//     pub in_unknown_protos:Load,
//     pub out_broadcast_pkts:Load,
//     pub out_discards:Load,
//     pub out_errors:Load,
//     pub out_multicast_pkts:Load,
//     pub out_octets:Load,
//     pub out_unicast_pkts:Load
// }

// impl StratumLoad {
//     pub fn new() -> Self {
//         StratumLoad {
//             in_broadcast_pkts: Load::new(),
//             in_discards: Load::new(),
//             in_errors: Load::new(),
//             in_fcs_errors: Load::new(),
//             in_multicast_pkts: Load::new(),
//             in_octets: Load::new(),
//             in_unicast_pkts: Load::new(),
//             in_unknown_protos: Load::new(),
//             out_broadcast_pkts: Load::new(),
//             out_discards: Load::new(),
//             out_errors: Load::new(),
//             out_multicast_pkts: Load::new(),
//             out_octets: Load::new(),
//             out_unicast_pkts: Load::new()
//         }
//     }

//     pub fn update(&mut self, v:[i64;14]) {
//         self.in_broadcast_pkts.update_packets(v[0]);
//         self.in_discards.update_packets(v[1]);
//         self.in_errors.update_packets(v[2]);
//         self.in_fcs_errors.update_packets(v[3]);
//         self.in_multicast_pkts.update_packets(v[4]);
//         self.in_octets.update_bytes(v[5]);
//         self.in_unicast_pkts.update_packets(v[6]);
//         self.in_unknown_protos.update_packets(v[7]);
//         self.out_broadcast_pkts.update_packets(v[8]);
//         self.out_discards.update_packets(v[9]);
//         self.out_errors.update_packets(v[10]);
//         self.out_multicast_pkts.update_packets(v[11]);
//         self.out_octets.update_bytes(v[12]);
//         self.out_unicast_pkts.update_packets(v[13]);
//     }
// }