use super::Service;
use super::{
    request::{DefaultRequest, Request},
    ParseRequest,
};
use futures::future;
use std::marker::PhantomData;
use tower::Service as towerService;

pub(crate) struct _TowerServiceWrap<T> {
    pub inner: T,
}

impl<T, R> tower::Service<Request<R>> for _TowerServiceWrap<T>
where
    T: Service<Request = R> + Send + Sync,
{
    type Response = Option<usize>;

    type Error = crate::error::MyError;

    type Future = futures::future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<R>) -> Self::Future {
        futures::future::ready(self.inner.process(req))
    }
}

pub(crate) struct _TowerService_DecodeRequest<T, R> {
    pub(crate) inner: T,
    pub(crate) pha: std::marker::PhantomData<R>,
}

impl<T, R> _TowerService_DecodeRequest<T, R> {
    pub fn new(t: T) -> _TowerService_DecodeRequest<T, R> {
        _TowerService_DecodeRequest {
            inner: t,
            pha: PhantomData,
        }
    }
}

impl<T, R> towerService<Request<DefaultRequest>> for _TowerService_DecodeRequest<T, R>
where
    T: towerService<Request<R>>,
    R: Send + ParseRequest,
    T::Error: From<crate::error::MyError>,
{
    type Response = T::Response;

    type Error = T::Error;

    type Future = futures::future::Either<
        T::Future,
        futures::future::Ready<std::result::Result<T::Response, T::Error>>,
    >;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<DefaultRequest>) -> Self::Future {
        match req.parse() {
            Ok(req) => futures::future::Either::Left(self.inner.call(req)),
            Err(err) => futures::future::Either::Right(future::ready(Err(err.into()))),
        }
    }
}
