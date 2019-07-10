use crate::app::common::{CommonOperation, CommonState, MergeResult};
use crate::app::p4App;
use crate::representation::Device;
use crate::context::ContextHandle;
use crate::event::{Event, CommonEvents};
use serde::export::PhantomData;

pub trait p4AppExtended<E> {

}

pub struct p4AppExtendedCore<A, E> {
    common:CommonState,
    extension: A,
    phantom:PhantomData<E>
}

impl<A, E> CommonOperation<E> for p4AppExtendedCore<A, E> where E:Event {
    fn merge_device(&mut self, info: Device, ctx:&ContextHandle<E>) -> MergeResult {
        let result = self.common.merge_device(info, ctx);
        result
    }
}

impl<A, E> p4App<E> for p4AppExtendedCore<A, E>
    where A:p4AppExtended<E>, E:Event
{

}

pub struct ExampleExtended {

}

impl p4AppExtended<CommonEvents> for ExampleExtended {

}

pub fn extend<E:Event,A:p4AppExtended<E>>(app: A) ->p4AppExtendedCore<A, E>{
    p4AppExtendedCore {
        common: CommonState::new(),
        extension: app,
        phantom: PhantomData
    }
}