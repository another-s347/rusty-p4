use futures::StreamExt;

use crate::util::flow::Flow;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::{future::Future, task::Poll};
use pin_project::pin_project;

pub mod flow;
// pub mod packet;
pub mod publisher;
pub mod value;

pub fn hash<T>(obj: T) -> u64
where
    T: Hash,
{
    let mut hasher = DefaultHasher::new();
    obj.hash(&mut hasher);
    hasher.finish()
}

pub struct FinishSignal {
    inner: tokio::sync::oneshot::Receiver<()>,
}

impl FinishSignal {
    pub fn new(inner: tokio::sync::oneshot::Receiver<()>) -> Self {
        Self { inner }
    }
}

impl std::future::Future for FinishSignal {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match unsafe { Pin::new_unchecked(&mut self.inner) }.poll(cx) {
            Poll::Ready(_) => Poll::Ready(()),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[pin_project]
pub struct SizeHintStream<S> {
    #[pin]
    pub inner: S,
    pub size_hint: Option<usize>
}

impl<S> futures::Stream for SizeHintStream<S> where S: futures::Stream {
    type Item = S::Item;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let inner = this.inner;
        match inner.poll_next(cx) {
            Poll::Ready(r) => Poll::Ready(r),
            Poll::Pending => Poll::Pending
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.size_hint)
    }
}

/// A boxed `Service + Send` trait object.
///
/// [`BoxService`] turns a service into a trait object, allowing the response
/// future type to be dynamic. This type requires both the service and the
/// response future to be [`Send`].
///
/// See module level documentation for more details.
pub(crate) struct BoxService<T, U, E> {
    inner:
        Box<dyn tower::Service<T, Response = U, Error = E, Future = BoxFuture<U, E>> + Send + Sync>,
}

/// A boxed `Future + Send` trait object.
///
/// This type alias represents a boxed future that is [`Send`] and can be moved
/// across threads.
type BoxFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send + Sync>>;

#[derive(Debug)]
struct Boxed<S> {
    inner: S,
}

impl<T, U, E> BoxService<T, U, E> {
    #[allow(missing_docs)]
    pub fn new<S>(inner: S) -> Self
    where
        S: tower::Service<T, Response = U, Error = E> + Send + Sync + 'static,
        S::Future: Send + Sync + 'static,
    {
        let inner = Box::new(Boxed { inner });
        BoxService { inner }
    }
}

impl<T, U, E> tower::Service<T> for BoxService<T, U, E> {
    type Response = U;
    type Error = E;
    type Future = BoxFuture<U, E>;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Result<(), E>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: T) -> BoxFuture<U, E> {
        self.inner.call(request)
    }
}

impl<S, Request> tower::Service<Request> for Boxed<S>
where
    S: tower::Service<Request> + 'static,
    S::Future: Send + Sync + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<S::Response, S::Error>> + Sync + Send>>;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        Box::pin(self.inner.call(request))
    }
}
