use crate::context::Context;
use crate::app::common::CommonOperation;
use hyper::{Server as HyperServer, Request, Body, Response};
use hyper::server::Builder as HyperBuilder;
use hyper::server::conn::AddrIncoming;
use hyper::service::service_fn_ok;
use hyper::rt::{self, spawn};
use std::net::SocketAddr;
use futures::prelude::*;

pub struct Netconfig {

}

impl Netconfig {
    pub fn from_json() -> Netconfig {
        unimplemented!()
    }
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
                    let json_body:serde_json::Value = serde_json::from_slice(x.as_ref()).unwrap();
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