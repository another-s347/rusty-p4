use crate::context::ContextHandle;
use crate::event::{Event, CoreRequest, CommonEvents};
use std::time::Duration;
use futures::prelude::*;
use futures03::prelude::*;
use log::{info, trace, warn, debug, error};
use crate::util::flow::{Flow, FlowTable, FlowAction};
use crate::util::value::{Value, MAC};
use crate::util::packet::Ethernet;
use crate::util::packet::Packet;
use bytes::Bytes;
use crate::util::packet::data::Data;

pub fn on_probe_received<E>(device_name:String, data:Data ,ctx:&ContextHandle<E>) where E:Event {
    let probe:Result<serde_json::Value,serde_json::Error> = serde_json::from_slice(&data.0);
    let from:Option<&str> = probe.iter()
        .flat_map(|x|x.as_object())
        .flat_map(|x|x.get("device"))
        .flat_map(|x|x.as_str())
        .next();
    if let Some(from) = from {
        // info!(target:"linkprobe","link detect {}<->{}",device_name,from);
        ctx.send_event(CommonEvents::LinkDetected(device_name,from.to_owned()).into());
    }
    else {
        error!(target:"linkprobe","invalid probe == {:?}",probe);
    }
}

pub fn on_device_added<E>(device_name:String,ctx:&ContextHandle<E>) where E:Event
{
    let mut task = tokio::timer::Interval::new_interval(Duration::new(3,0));
    new_probe_interceptor(&device_name,ctx);
    let probe = new_probe(&device_name).to_vec();
    let my_sender = ctx.sender.clone();
    tokio::spawn(async move {
        while let Some(s) = task.next().await {
            trace!(target:"linkprobe","device probe {}", &device_name);
            my_sender.unbounded_send(CoreRequest::PacketOut {
                device: device_name.clone(),
                port: 1,
                packet: probe.clone()
            });
            my_sender.unbounded_send(CoreRequest::PacketOut {
                device: device_name.clone(),
                port: 2,
                packet: probe.clone()
            });
            my_sender.unbounded_send(CoreRequest::PacketOut {
                device: device_name.clone(),
                port: 3,
                packet: probe.clone()
            });
        }
    });
}

pub fn new_probe_interceptor<E>(device_name:&str,ctx:&ContextHandle<E>) where E:Event {
    let flow = Flow {
        device: device_name.to_owned(),
        table: FlowTable {
            name: "IngressPipeImpl.acl",
            matches: &[("hdr.ethernet.ether_type",Value::EXACT(0x861u16))]
        },
        action: FlowAction {
            name: "send_to_cpu",
            params: &[]
        },
        priority: 4000,
        metadata: 0
    };
    ctx.insert_flow(flow);
}

pub fn new_probe(device_name:&str) -> Bytes
{
    let probe = json!({
        "device": device_name,
    });
    Ethernet {
        src: MAC([0x12,0x34,0x56,0x12,0x34,0x56]),
        dst: MAC::broadcast(),
        ether_type: 0x861,
        payload: Data(Bytes::from(serde_json::to_vec(&probe).unwrap()))
    }.into_bytes()
}