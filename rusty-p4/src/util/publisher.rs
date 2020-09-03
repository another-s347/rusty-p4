use parking_lot::RwLock;
use async_trait::async_trait;

#[async_trait]
pub trait Handler<E>:'static + Send + Sync {
    async fn handle(&self, event: E);
}

pub struct Publisher<E> {
    handlers: RwLock<Vec<Box<dyn Handler<E>>>>
}

impl<E> Default for Publisher<E> {
    fn default() -> Self {
        Publisher {
            handlers: RwLock::new(Vec::new()),
        }
    }
}

impl<E> Publisher<E> where E:Clone + 'static {
    pub async fn emit(&self, event: E) {
        for x in self.handlers.read().iter() {
            x.handle(event.clone()).await;
        }
    }

    pub fn add_handler<H>(&self, handler: H) where H: Handler<E> {
        self.handlers.write().push(Box::new(handler));
    }
}