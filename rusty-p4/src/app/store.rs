use std::{any::{Any, TypeId}, collections::HashMap, sync::Arc};
use super::App;
use super::Dependencies;
use futures::future::BoxFuture;

pub trait AppStore {
    fn store<T>(&mut self, object: T) -> Arc<T> where T: App + Clone + 'static;

    fn get<T>(&self) -> Option<T> where T: App + Clone + 'static;
}

pub fn install<S, T>(store: &mut S, option: T::Option) -> Arc<T>
where T:App + Clone, S: AppStore
{
    let dependencies: T::Dependency = T::Dependency::get(store);
    let app = T::init(dependencies, store, option);
    store.store(app)
}

#[derive(Default)]
pub struct DefaultAppStore {
    pub map: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
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

    fn get<T>(&self) -> Option<T> where T: App + Clone + 'static {
        self.map.get(&TypeId::of::<T>()).map(|x|{
            let ret: Arc<T> = x.clone().downcast().unwrap();
            let ret:&T = ret.as_ref();
            ret.clone()
        })
    }
}