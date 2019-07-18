use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use bytes::{BufMut, BytesMut};
use futures03::channel::mpsc::UnboundedSender;
use futures03::compat::*;
use futures03::FutureExt;
use futures03::prelude::*;
use futures03::stream::Stream;
use futures03::task::SpawnExt;
use futures::future::ok;
use hyper::{Body, Request, Response, Server as HyperServer, Server};
use hyper::body::Payload;
use hyper::rt::{self, spawn};
use hyper::server::Builder as HyperBuilder;
use hyper::server::conn::{AddrIncoming, AddrStream};
use hyper::service::{make_service_fn, MakeService, service_fn};
use log::{debug, error, info, trace, warn};
use serde::{Deserialize, Serialize};

use crate::context::{Context, ContextHandle};
use crate::event::{CoreRequest, Event, CommonEvents};
use crate::p4rt::bmv2::Bmv2SwitchConnection;
use crate::representation::{Device, DeviceType, Interface, Port};
use crate::util::value::MAC;

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
                    mac: Some(MAC::of(&port.interface.mac))
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
                    socket_addr: device_config.basic.socket_addr.clone(),
                    device_id: device_config.basic.device_id
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
        let http_builder = Server::bind(&([127, 0, 0, 1], 1818).into());
        NetconfigServer {
            http_builder
        }
    }
}

async fn hello(_: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    Ok(Response::new(Body::from("Hello World!")))
}

pub async fn build_netconfig_server<E>(server: NetconfigServer, core_event_sender:UnboundedSender<CoreRequest<E>>)
    where E:Event+Clone+Send+'static
{
    let server = server.http_builder.serve(make_service_fn(move|_|{
        let s = core_event_sender.clone();
        async move {
            let p = s.clone();
            Ok::<_, hyper::Error>(service_fn(move |req:Request<Body>| {
                let body = req.into_body();
                let len = body.content_length().unwrap();
                let buffer = BytesMut::with_capacity(len as usize);
                let y = p.clone();
                rt::spawn(body.fold(buffer,|mut x,y|{
                    let mut c=y.unwrap().into_bytes();
                    x.put(c);
                    futures03::future::ready(x)
                }).map(move|x|{
                    let config:Netconfig = serde_json::from_slice(x.as_ref()).unwrap();
                    for device in config.to_devices() {
                        y.unbounded_send(CoreRequest::AddDevice {
                            device,
                            reply: None,
                        }).unwrap();
                    }
                }));
                futures03::future::ok::<Response<Body>, hyper::Error>(Response::new(Body::from("Hello World!")))
            }))
        }
    }));

    server.await.unwrap();
}