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
use crate::util::BoxService;
use tower::Service as towerService;
use crate::app::options;
use crate::error::Result;

pub mod dummy;
pub mod server;
pub mod request;
mod tower_service;

pub use request::*;
use tower_service::*;

pub use self::server::Server;

/// `Service` is used to expose an application to various northbound api.
/// The app implement the trait `Service` to `process` request and produce a stream of response.
/// Northbound api might be implemented as multiple different backend.
/// The service is compatiable to tower eco system.
pub trait Service {
    /// Your request type must be parsed from [DefaultRequest], which has:
    /// - a path: a Vec<String>.
    /// - a action: a String like 'get' or 'set'
    /// - parameters: a HashMap<String, String>
    type Request: ParseRequest + Send;

    /// Name your service, which would be used as [Request::source].
    const NAME: &'static str;

    /// process your request and send back response via [Request::respond], the return value `Option<usize>` is the size hint (upper bound) of your response stream.
    fn process(&mut self, request: Request<Self::Request>) -> Result<Option<usize>>;
}

#[derive(Clone)]
pub struct ServiceBus {
    services: Arc<DashMap<&'static str, BoxService<Request<DefaultRequest>, Option<usize>, crate::error::MyError>>>,
}

impl ServiceBus {
    pub fn new() -> ServiceBus {
        let services = Arc::new(DashMap::new());
        ServiceBus {
            services,
        }
    }

    pub fn install_service<T>(&self, service: T)
    where T: Service + Send + Sync + 'static, T::Request: Sync
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

    pub async fn send<E: Server>(&self, target: &str, request: DefaultRequest, option: RequestOption) -> Result<impl futures::stream::Stream<Item=E::EncodeTarget>> {
        if let Some(mut s) = self.services.get_mut(target) {
            let mut service = (&mut *s);
            let (s, r) = tokio::sync::mpsc::channel(option.queue_size_hint);
            let request = Request {
                source: E::NAME,
                target: target.to_owned(),
                inner: request,
                channel: s,
                option
            };
            let response: Option<usize> = service.call(request).await?;
            let s = ReceiverStream::new(r).map(|x|{
                E::encode(x)
            });
            return Ok(crate::util::SizeHintStream {
                inner: s,
                size_hint: response
            })
        }
        else {
            return Err(crate::error::ServiceError::ServiceNotFound(target.to_owned()).into())
        }
    }
}