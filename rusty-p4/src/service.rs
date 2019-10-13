//use crate::app::sync_app::AsyncWrap;
use crate::app::Example;
use std::any::Any;
use std::cell::{Ref, RefCell};
use std::ops::Deref;
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};

pub trait Service {
    type ServiceType:Clone;

    fn get_service(&mut self) -> Self::ServiceType;
}