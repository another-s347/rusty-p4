use std::{any::{Any, TypeId}, collections::HashMap, sync::Arc};
use crate::util::publisher::Handler;

use super::App;
use super::Dependencies;
use futures::future::BoxFuture;
use downcast_rs::DowncastSync;

/// The trait defines how should we store many apps.
pub trait AppStore {
    fn store<T>(&mut self, object: T) -> Arc<T> where T: App + Clone + 'static;

    fn get<T>(&self) -> Option<T> where T: App + Clone + 'static;

    fn store_handler<T, E>(&mut self, app: T) where T: Handler<E>,E:'static;

    fn get_handlers<E>(&self) -> Vec<Arc<dyn crate::util::publisher::Handler<E>>> where E:'static;
}

pub fn install<S, T>(store: &mut S, option: T::Option) -> Option<Arc<T>>
where T:App + Clone, S: AppStore
{
    let dependencies: T::Dependency = T::Dependency::get(store)?;
    let app = T::init(dependencies, store, option);
    Some(store.store(app))
}

/// The default implementation of `AppStore` which should just works.
#[derive(Default)]
pub struct DefaultAppStore {
    pub map: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
    pub handler_map: HashMap<TypeId, Vec<Arc<dyn Any+Send+Sync>>>,
    pub join_handle: Vec<BoxFuture<'static, ()>>
}

impl AppStore for  DefaultAppStore {
    fn store<T>(&mut self, object: T) -> Arc<T> where T: App + Clone + 'static {
        let b = Arc::new(object);
        let ret = b.clone();

        let b = b as Arc<dyn Any + Send + Sync>;
        self.map.insert(TypeId::of::<T>(), b);
        ret
    }

    fn get<T>(&self) -> Option<T> where T: App + 'static {
        T::from_inner(self.map.get(&TypeId::of::<T::Container>()).map(|x|{
            let ret: Arc<T::Container> = x.clone().downcast().unwrap();
            let ret:T::Container = ret.as_ref().clone();
            ret
        }))
    }

    fn store_handler<T, E>(&mut self, app: T) where T: Handler<E>,E:'static {
        let a = Arc::new(app) as Arc<dyn Handler<E>>;

        let a = Arc::new(a) as Arc<dyn Any + Send + Sync>;
        self.handler_map.insert(TypeId::of::<E>(), vec![a]);
    }

    fn get_handlers<E>(&self) -> Vec<Arc<dyn crate::util::publisher::Handler<E>>> where E:'static {
        let a = self.handler_map.get(&TypeId::of::<E>()).unwrap();
        a.iter().filter_map(|x|{
            x.clone().downcast::<Arc<dyn crate::util::publisher::Handler<E>>>().map(|x|{
                x.as_ref().clone()
            }).ok()
        }).collect::<Vec<_>>()
    }
}