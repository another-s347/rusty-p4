use std::net::SocketAddr;

use futures::prelude::*;
use hyper::{Body, Request, Response, Server as HyperServer};
use hyper::rt::{self, spawn};
use hyper::server::Builder as HyperBuilder;
use hyper::server::conn::AddrIncoming;
use hyper::service::service_fn_ok;

use crate::app::common::CommonOperation;
use crate::context::Context;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct Netconfig {
    devices: HashMap<String, NetconfigDevice>,
    ports: HashMap<String, NetconfigDeviceInterface>
}

#[derive(Deserialize, Debug)]
pub struct NetconfigDevice {
    basic: NetconfigDeviceBasic,
    ports: HashMap<String, NetconfigDevicePort>
}

#[derive(Deserialize, Debug)]
pub struct NetconfigDeviceBasic {
    managementAddress: String,
    driver: String,
    pipeconf: String
}

#[derive(Deserialize, Debug)]
pub struct NetconfigDevicePort {
    number: u32,
    enabled: bool
}

#[derive(Deserialize, Debug)]
pub struct NetconfigDeviceInterface {
    mac: String,
    name: String,
}

pub struct NetconfigServer {
    http_builder: HyperBuilder<AddrIncoming>
}

impl NetconfigServer {
    pub fn new() {

    }
}

pub fn run_netconfig<T:CommonOperation>(server: NetconfigServer, state: &mut T)
{
    let server = server.http_builder.serve(||{
        service_fn_ok(move |req:Request<Body>|{
            rt::spawn(
                req.into_body().concat2().map(|x|{
                    let config:Netconfig = serde_json::from_slice(x.as_ref()).unwrap();
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