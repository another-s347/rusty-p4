use crate::util::flow::Flow;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::{task::Poll, future::Future};

pub mod flow;
pub mod packet;
pub mod value;
pub mod publisher;

pub fn hash<T>(obj: T) -> u64
where
    T: Hash,
{
    let mut hasher = DefaultHasher::new();
    obj.hash(&mut hasher);
    hasher.finish()
}

pub struct FinishSignal {
    inner: tokio::sync::oneshot::Receiver<()>
}

impl FinishSignal {
    pub fn new(inner: tokio::sync::oneshot::Receiver<()>) -> Self {
        Self {
            inner
        }
    }
}

impl std::future::Future for FinishSignal {
    type Output = ();

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        match unsafe { Pin::new_unchecked(&mut self.inner) }.poll(cx) {
            Poll::Ready(_) => Poll::Ready(()),
            Poll::Pending => Poll::Pending
        }
    }
}