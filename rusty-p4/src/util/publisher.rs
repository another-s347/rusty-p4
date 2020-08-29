use std::sync::RwLock;

pub trait Handler<E>:'static + Send + Sync {
    fn handle(&self, event: E);
}

pub struct Publisher<E> {
    handlers: RwLock<Vec<Box<dyn Handler<E>>>>
}

impl<E> Publisher<E> where E:Clone + 'static {
    pub fn emit(&self, event: E) {
        self.handlers.read().unwrap().iter().for_each(|x|{
            x.handle(event.clone());
        });
    }

    pub fn add_handler<H>(&self, handler: H) where H: Handler<E> {
        self.handlers.write().unwrap().push(Box::new(handler));
    }
}