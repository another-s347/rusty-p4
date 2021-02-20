use crate::app::App;

pub struct System<A> {
    app: A,
}

impl<A> System<A>
where
    A: App,
{
    pub async fn run_to_end(&self) {
        // self.app.join_handle().await;
    }
}
