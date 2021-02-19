use tuple_list::tuple_list_type;
use std::sync::Arc;
use crate::util::publisher::Handler;

#[derive(Clone)]
pub struct PacketService {
    bmv2: Option<crate::p4rt::bmv2::Bmv2Manager>,
    publisher: Arc<crate::util::publisher::Publisher<PacketEvent>>
}

#[derive(Clone)]
pub struct PacketEvent {

}

impl crate::app::NewApp for PacketService {
    type Dependency = tuple_list_type!(Option<crate::p4rt::bmv2::Bmv2Manager>);

    type Option = ();

    fn init<S>(dependencies: Self::Dependency, store: &mut S, option: Self::Option) -> Self where S: super::store::AppStore  {
        todo!()
    }
}

impl PacketService {
    pub fn subscribe<T>(&self, handler: T) where T: Handler<PacketEvent> {
        self.publisher.add_handler(handler);
    }
}