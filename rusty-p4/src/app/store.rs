use std::{any::{Any, TypeId}, collections::HashMap, sync::Arc};

pub trait AppStore {
    fn store<T>(&mut self, object: &T) where T: Clone + Sync + Send + 'static;

    fn get<T>(&self) -> Option<T> where T: Clone + Sync + Send + 'static;
}

pub struct DefaultAppStore {
    pub map: HashMap<TypeId, Arc<dyn Any + Send + Sync>>
}

impl AppStore for  DefaultAppStore {
    fn store<T>(&mut self, object: &T) where T: Clone + Sync + Send + 'static {
        let b = Arc::new(object.clone());

        let b = b as Arc<dyn Any + Send + Sync>;
        self.map.insert(TypeId::of::<T>(), b);
    }

    fn get<T>(&self) -> Option<T> where T: Clone + Sync + Send + 'static {
        self.map.get(&TypeId::of::<T>()).map(|x|{
            let ret: Arc<T> = x.clone().downcast().unwrap();
            let ret:&T = ret.as_ref();
            ret.clone()
        })
    }
}