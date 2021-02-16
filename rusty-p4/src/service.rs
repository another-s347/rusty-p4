//use crate::app::sync_app::AsyncWrap;
use std::{any::Any, collections::HashMap, marker::PhantomData, pin::Pin};
use std::cell::{Ref, RefCell};
use std::ops::Deref;
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};

use bytes::Bytes;
use dashmap::DashMap;
use futures::{StreamExt, future::BoxFuture, stream::BoxStream};
use serde::{Serialize, de::DeserializeOwned, Deserialize};
use async_trait::async_trait;
use tokio_stream::wrappers::ReceiverStream;
use tower::util::BoxService;
use tower::Service as towerService;
use crate::app::options;

#[cfg(test)]
pub mod dummy;
pub mod server;
pub mod request;
pub mod tower_service;

pub use request::*;
use tower_service::*;

use self::server::Server;

/// `Service` is used to expose an application to various northbound api.
/// The app implement the trait `Service` to `process` request and produce a stream of response.
/// Northbound api might be implemented as multiple different backend.
/// The service is compatiable to tower eco system.
pub trait Service {
    /// Your request type must be deserialized from `DefaultRequest`, which has:
    /// - a path: a Vec<String>.
    /// - a action: a String like 'get' or 'set'
    /// - parameters: a HashMap<String, String>
    type Request: ParseRequest + Send;

    const NAME: &'static str;

    /// process your request and send back response via request.respond(), the return value `Option<usize>` is the size hint (upper bound) of your response stream.
    fn process(&mut self, request: Request<Self::Request>) -> std::io::Result<Option<usize>>;
}

pub struct ServiceBus {
    services: Arc<DashMap<&'static str, BoxService<Request<DefaultRequest>, Option<usize>, std::io::Error>>>,
}

impl ServiceBus {
    pub fn new() -> ServiceBus {
        let services = Arc::new(DashMap::new());
        ServiceBus {
            services,
        }
    }

    pub fn install_service<T>(&self, service: T)
    where T: Service + Send + Sync + 'static
    {
        if self.services.contains_key(T::NAME) {
            todo!()
        }
        let wrapper = _TowerServiceWrap {
            inner: service
        };
        let b = BoxService::new(_TowerService_DecodeRequest {
            inner: wrapper,
            pha: PhantomData
        });
        self.services.insert(T::NAME, b);
    }

    pub async fn send<E: Server>(&self, target: &'static str, request: DefaultRequest, option: RequestOption) -> std::io::Result<impl futures::stream::Stream<Item=E::EncodeTarget>> {
        if let Some(mut s) = self.services.get_mut(target) {
            let mut service = (&mut *s);
            let (s, r) = tokio::sync::mpsc::channel(option.queue_size_hint);
            let request = Request {
                source: E::NAME,
                target,
                inner: request,
                channel: s,
                option
            };
            let response: Option<usize> = service.call(request).await?;
            return Ok(ReceiverStream::new(r).map(|x|{
                E::encode(x)
            }));
        }
        else {
            return Err(std::io::ErrorKind::NotFound.into())
        }
    }
}