use crate::app::p4App;

pub trait p4AppExtended {

}

pub struct p4AppExtendedCore<E> {
    extension: E
}

impl<E> p4App for p4AppExtendedCore<E>
    where E:p4AppExtended
{

}