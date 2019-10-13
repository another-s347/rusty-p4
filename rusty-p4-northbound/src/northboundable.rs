use futures::future::MapErr;
use futures::task::SpawnExt;
use futures::{future, SinkExt, TryFutureExt, StreamExt};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Version};
use rusty_p4::app::{Example, P4app};
use rusty_p4::event::Event;
//use rusty_p4::service::Service;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::pin::Pin;
use tokio::future::Future;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use tower_service::Service as towerService;
use std::sync::Arc;
use std::collections::HashMap;
use rusty_p4::context::ContextHandle;


// app/[app name]/[command]&args
#[derive(Clone)]
pub struct NorthboundServer {
//    apps: HashMap<&'static str,Arc<dyn Northboundable>>,
    channel: UnboundedSender<rusty_p4::event::NorthboundRequest>
}

async fn hyper_service(server: NorthboundServer, req: Request<Body>) -> Result<Response<Body>,hyper::Error> {
    match (req.method(), req.uri().path()) {
        (&hyper::Method::GET, path) => {
            let split = path.split('/');
            
        }
        _ => {

        }
    }
    unimplemented!()
}

impl NorthboundServer {
    pub fn run(&self) {
        let addr = ([127, 0, 0, 1], 3000).into();

        let make_svc = make_service_fn(|c| {
            let nb_server = self.clone();
            futures::future::ok::<_, hyper::Error>(service_fn(move |_req| {
                hyper_service(nb_server.clone(), _req)
            }))
        });

        let server = hyper::Server::bind(&addr)
            .serve(make_svc)
            .map_err(|e| eprintln!("server error: {}", e));
    }
}