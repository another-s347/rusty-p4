use crate::app::async_app::ExampleAsyncApp;
use crate::app::sync_app::AsyncWrap;
use crate::app::Example;
use std::any::Any;
use std::cell::{Ref, RefCell};
use std::ops::Deref;
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};

pub enum Service<T> {
    Async(Arc<T>),
    AsyncFromSyncWrap(Arc<Mutex<T>>),
    SyncFromAsyncWrap(Rc<RefCell<AsyncWrap<T>>>),
    Sync(Rc<RefCell<T>>),
}

impl<T> Clone for Service<T> {
    fn clone(&self) -> Self {
        match self {
            Service::Async(i) => Service::Async(i.clone()),
            Service::AsyncFromSyncWrap(i) => Service::AsyncFromSyncWrap(i.clone()),
            Service::SyncFromAsyncWrap(i) => Service::SyncFromAsyncWrap(i.clone()),
            Service::Sync(i) => Service::Sync(i.clone()),
        }
    }
}

pub enum DefaultServiceStorage {
    Async(Arc<dyn Any + Send + Sync>),
    AsyncFromSyncWrap(Arc<dyn Any + Send + Sync>),
    SyncFromAsyncWrap(Rc<dyn Any>),
    Sync(Rc<dyn Any>),
}

pub enum ServiceGuard<'a, T> {
    Ref(Ref<'a, T>),
    Direct(&'a T),
    Mutex(MutexGuard<'a, T>),
}

impl<T> Service<T>
where
    T: 'static,
{
    pub fn get(&self) -> ServiceGuard<T> {
        self.try_get().unwrap()
    }

    pub fn try_get(&self) -> Option<ServiceGuard<T>> {
        Some(match self {
            Service::Sync(i) => ServiceGuard::Ref(i.borrow()),
            Service::Async(i) => ServiceGuard::Direct(i),
            Service::AsyncFromSyncWrap(i) => ServiceGuard::Mutex(i.lock().ok()?),
            Service::SyncFromAsyncWrap(i) => ServiceGuard::Ref(Ref::map(i.borrow(), |x| &x.inner)),
        })
    }

    pub fn to_sync_storage(self) -> DefaultServiceStorage {
        match self {
            Service::Sync(i) => DefaultServiceStorage::Sync(i),
            Service::SyncFromAsyncWrap(i) => DefaultServiceStorage::SyncFromAsyncWrap(i),
            _ => unreachable!(),
        }
    }
}

impl<T> Service<T>
where
    T: 'static + Send + Sync,
{
    pub fn to_async_storage(self) -> DefaultServiceStorage {
        match self {
            Service::Async(i) => DefaultServiceStorage::Async(i),
            Service::AsyncFromSyncWrap(i) => DefaultServiceStorage::AsyncFromSyncWrap(i),
            _ => unreachable!(),
        }
    }
}

impl<T> Service<T>
where
    T: 'static + Send,
{
    pub fn to_sync_wrap_storage(self) -> DefaultServiceStorage {
        match self {
            Service::AsyncFromSyncWrap(i) => DefaultServiceStorage::AsyncFromSyncWrap(i),
            _ => unreachable!(),
        }
    }
}

impl<'a, T> Deref for ServiceGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            ServiceGuard::Ref(i) => i,
            ServiceGuard::Direct(i) => i,
            ServiceGuard::Mutex(i) => i,
        }
    }
}

pub trait ServiceStorage<T> {
    fn to_service(&self) -> Option<Service<T>>;
}

default impl<T> ServiceStorage<T> for DefaultServiceStorage
where
    T: 'static,
{
    fn to_service(&self) -> Option<Service<T>> {
        match self {
            DefaultServiceStorage::Sync(i) => Some(Service::Sync(
                Rc::downcast::<RefCell<T>>(i.clone()).unwrap(),
            )),
            DefaultServiceStorage::SyncFromAsyncWrap(i) => Some(Service::SyncFromAsyncWrap(
                Rc::downcast::<RefCell<AsyncWrap<T>>>(i.clone()).unwrap(),
            )),
            DefaultServiceStorage::Async(i) => None,
            DefaultServiceStorage::AsyncFromSyncWrap(i) => None,
        }
    }
}

impl<T> ServiceStorage<T> for DefaultServiceStorage
where
    T: Send + Sync + 'static,
{
    fn to_service(&self) -> Option<Service<T>> {
        Some(match self {
            DefaultServiceStorage::Sync(i) => {
                Service::Sync(Rc::downcast::<RefCell<T>>(i.clone()).unwrap())
            }
            DefaultServiceStorage::SyncFromAsyncWrap(i) => Service::SyncFromAsyncWrap(
                Rc::downcast::<RefCell<AsyncWrap<T>>>(i.clone()).unwrap(),
            ),
            DefaultServiceStorage::Async(i) => {
                Service::Async(Arc::downcast::<T>(i.clone()).unwrap())
            }
            DefaultServiceStorage::AsyncFromSyncWrap(i) => {
                Service::AsyncFromSyncWrap(Arc::downcast::<Mutex<T>>(i.clone()).unwrap())
            }
        })
    }
}

#[test]
fn test_SyncFromAsyncWrap_storage() {
    let t = ExampleAsyncApp::new();
    let service_t = Service::SyncFromAsyncWrap(Rc::new(RefCell::new(AsyncWrap::new(t))));
    let storage_t = service_t.to_sync_storage();
    let get_t: Service<ExampleAsyncApp> = storage_t.to_service().unwrap();
    get_t.get().test();
}

#[test]
fn test_Async_storage() {
    let t = ExampleAsyncApp::new();
    let service_t = Service::Async(Arc::new(t));
    let storage_t = service_t.to_async_storage();
    let get_t: Service<ExampleAsyncApp> = storage_t.to_service().unwrap();
    get_t.get().test();
}

#[test]
fn test_AsyncFromSyncWrap_storage() {
    let t = Example { counter: 0 };
    let service_t = Service::AsyncFromSyncWrap(Arc::new(Mutex::new(t)));
    let storage_t = service_t.to_async_storage();
    let get_t: Service<Example> = storage_t.to_service().unwrap();
    get_t.get().test();
}

#[test]
fn test_Sync_storage() {
    let t = Example { counter: 0 };
    let service_t = Service::Sync(Rc::new(RefCell::new(t)));
    let storage_t = service_t.to_sync_storage();
    let get_t: Service<Example> = storage_t.to_service().unwrap();
    get_t.get().test();
}
