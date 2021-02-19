use std::collections::HashMap;
use serde::{Serialize, Deserialize};
pub struct Request<T> {
    pub source: &'static str,
    pub target: String,
    pub inner: T,
    pub channel: tokio::sync::mpsc::Sender<Box<dyn erased_serde::Serialize + Send>>,
    pub option: RequestOption
}

impl<T> Request<T> {
    /// Respond to backend. The response need to be serde::Serialize; 
    pub async fn respond<M>(&self, msg: M) where M: Serialize + 'static + Send {
        self.channel.send(Box::new(msg)).await;
    }
}

impl Request<DefaultRequest> {
    pub fn parse<T>(self) -> crate::error::Result<Request<T>> where T:ParseRequest {
        Ok(Request {
            source: self.source,
            target: self.target,
            inner: T::parse(self.inner)?,
            channel: self.channel,
            option: self.option,
        })
    }
}

#[derive(Serialize, Deserialize)]
pub struct DefaultRequest {
    pub path: Vec<String>,
    pub action: String,
    pub params: HashMap<String, String>
}

/// Options for your request, may be processed by different services or layers.
#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct RequestOption {
    /// size for the mpsc channel.
    #[serde(default= "_default_1")]
    pub queue_size_hint: usize
}

impl Default for RequestOption {
    fn default() -> Self {
        Self {
            queue_size_hint: 1,
        }
    }
}

pub trait ParseRequest: Sized {
    fn parse(req: DefaultRequest) -> crate::error::Result<Self>;
}

impl ParseRequest for DefaultRequest {
    fn parse(req: DefaultRequest) -> crate::error::Result<Self> {
        Ok(req)
    }
}

// https://github.com/serde-rs/serde/issues/1030
fn _default_1() -> usize { 1 }