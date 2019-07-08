use crate::app::common::{CommonOperation, CommonState, MergeResult};
use crate::app::p4App;
use crate::representation::Device;

pub trait p4AppExtended {

}

pub struct p4AppExtendedCore<E> {
    common:CommonState,
    extension: E
}

impl CommonOperation for p4AppExtended {
    fn merge_device(&mut self, info: Device) -> MergeResult {
        unimplemented!()
    }
}

impl<E> p4App for p4AppExtendedCore<E>
    where E:p4AppExtended
{

}