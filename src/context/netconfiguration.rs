use std::net::SocketAddr;

use futures::prelude::*;
use hyper::{Body, Request, Response, Server as HyperServer};
use hyper::rt::{self, spawn};
use hyper::server::Builder as HyperBuilder;
use hyper::server::conn::AddrIncoming;
use hyper::service::service_fn_ok;
use crate::app::common::CommonOperation;
use crate::context::{Context, ContextHandle};
use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use crate::representation::{Device, DeviceType, Port, Interface};
use crate::util::value::MAC;
use crate::event::{Event, CoreRequest};
use crate::p4rt::bmv2::Bmv2SwitchConnection;
use futures03::channel::mpsc::UnboundedSender;

#[derive(Deserialize, Debug)]
pub struct Netconfig {
    devices: HashMap<String, NetconfigDevice>
}

#[derive(Deserialize, Debug)]
pub struct NetconfigDevice {
    basic: NetconfigDeviceBasic,
    ports: HashMap<String, NetconfigDevicePort>
}

#[derive(Deserialize, Debug)]
pub struct NetconfigDeviceBasic {
    socket_addr:String,
    device_id: u64,
    driver: String,
    pipeconf: String
}

#[derive(Deserialize, Debug)]
pub struct NetconfigDevicePort {
    number: u32,
    enabled: bool,
    interface: NetconfigDeviceInterface
}

#[derive(Deserialize, Debug)]
pub struct NetconfigDeviceInterface {
    mac: String,
    name: String,
}

impl Netconfig {
    pub fn to_devices(&self) -> Vec<Device> {
        self.devices.iter().map(|(name,device_config)|{
            let ports:HashSet<Port> = device_config.ports.iter().map(|(port_name,port)|{
                let interface = Interface {
                    name: port.interface.name.clone(),
                    ip: None,
                    mac: Some(MAC::of(port.interface.mac.clone()))
                };
                Port {
                    number: port.number,
                    interface: Some(interface)
                }
            }).collect();
            Device {
                name: name.clone(),
                ports,
                typ: DeviceType::MASTER {
                    management_address: device_config.basic.socket_addr.clone()
                },
                device_id: device_config.basic.device_id,
                index: 0
            }
        }).collect()
    }
}

pub struct NetconfigServer {
    http_builder: HyperBuilder<AddrIncoming>
}

impl NetconfigServer {
    pub fn new() -> NetconfigServer {
        unimplemented!()
    }
}

pub fn run_netconfig<E>(server: NetconfigServer, core_event_sender:UnboundedSender<CoreRequest<E>>)
    where E:Event+Clone+Send+'static
{
    let server = server.http_builder.serve(move||{
        let s= core_event_sender.clone();
        service_fn_ok(move|req:Request<Body>|{
            let s= s.clone();
            rt::spawn(
                req.into_body().concat2().map(move|x|{
                    let config:Netconfig = serde_json::from_slice(x.as_ref()).unwrap();
                    for device in config.to_devices() {
                        // TODO
                        s.unbounded_send(CoreRequest::AddDevice {
                            name: device.name,
                            address: "".to_string(),
                            device_id: 0,
                            reply: None
                        });
                    }
                }).map_err(|err|{
                    dbg!(err);
                })
            );
            Response::new(Body::from("Hello World!"))
        })
    }).map_err(|e|{
        eprintln!("server error: {}", e);
    });

    rt::spawn(server);
}