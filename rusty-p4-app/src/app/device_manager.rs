use std::{collections::HashMap, sync::Arc, sync::Mutex};

use async_trait::async_trait;

use crate::{representation::{Device, DeviceID}, util::publisher::Handler};

#[derive(Clone)]
pub struct DeviceManager {
    bmv2_manager: crate::p4rt::bmv2::Bmv2Manager,
    devices: Arc<Mutex<HashMap<DeviceID, Device>>>,
    event_publisher: Arc<crate::util::publisher::Publisher<DeviceEvent>>
}

#[derive(Clone)]
pub enum DeviceEvent {
    DeviceAdded(DeviceID)
}

#[async_trait]
impl crate::app::App for DeviceManager {
    type Container = Self;
    type Dependency = tuple_list::tuple_list_type!(crate::p4rt::bmv2::Bmv2Manager);

    type Option = ();

    const Name: &'static str = "DeviceManager";

    fn init<S>(dependencies: Self::Dependency, store: &mut S, option: Self::Option) -> Self where S: crate::app::store::AppStore {
        let tuple_list::tuple_list!(bmv2_manager) = dependencies;
        let app = Self {
            bmv2_manager: bmv2_manager.clone(),
            devices: Default::default(),
            event_publisher: Default::default()
        };

        bmv2_manager.subscribe_event(app.clone());
        store.store(app.clone());

        app
    }

    fn from_inner(app: Option<Self::Container>) -> Option<Self> {
        app
    }

    async fn run(&self) {
        todo!()
    }
}

#[async_trait]
impl Handler<crate::p4rt::bmv2::Bmv2Event> for DeviceManager {
    async fn handle(&self, event: crate::p4rt::bmv2::Bmv2Event) {
        todo!()
    }
}

impl DeviceManager {
    pub fn subscribe<T: Handler<DeviceEvent>>(&self, app: T) {
        self.event_publisher.add_handler(app);
    }

    pub fn get_device(&self, device: DeviceID) -> Device {
        self.devices.lock().unwrap().get(&device).unwrap().clone()
    }
}