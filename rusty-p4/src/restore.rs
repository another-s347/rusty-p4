use crate::context::{Context, ContextHandle};
use crate::event::{CoreRequest, Event};
use crate::representation::{Device, DeviceID};
use ctrlc;
use serde::{Deserialize, Serialize};
use serde_json::*;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::{Arc, Mutex};

#[derive(Serialize, Deserialize, Clone)]
pub struct RestoreState {
    pub devices: Vec<Device>,
}

impl RestoreState {
    pub fn new() -> RestoreState {
        RestoreState { devices: vec![] }
    }
}

#[derive(Clone)]
pub struct Restore {
    pub state: Arc<Mutex<RestoreState>>,
    pub file: PathBuf,
}

impl Restore {
    pub fn new<T: AsRef<Path>>(path: T) -> Restore {
        let path = path.as_ref();
        let state = if let Ok(f) = File::open(path) {
            serde_json::from_reader(f).unwrap_or(RestoreState::new())
        } else {
            RestoreState::new()
        };
        let obj = Restore {
            state: Arc::new(Mutex::new(state)),
            file: PathBuf::from(path),
        };
        let obj2 = obj.clone();
        ctrlc::set_handler(move || {
            obj2.save();
            println!("exiting...");
            exit(0);
        })
        .expect("Error setting Ctrl-C handler");
        obj
    }

    pub fn save(&self) {
        let mut f = File::create(self.file.as_path()).unwrap();
        serde_json::to_writer_pretty(f, self.state.as_ref());
    }

    pub fn restore<E>(&mut self, ctx: ContextHandle<E>)
    where
        E: Event,
    {
        let state: Vec<Device> = self.state.lock().unwrap().devices.drain(0..).collect();
        let sender = ctx.sender;
        for device in state {
            sender
                .unbounded_send(CoreRequest::AddDevice {
                    device,
                    reply: None,
                })
                .unwrap();
        }
    }

    pub fn add_device(&mut self, device: Device) {
        let mut state = self.state.lock().unwrap();
        state.devices.push(device);
    }

    pub fn remove_device(&mut self, deviceID: DeviceID) {
        let mut state = self.state.lock().unwrap();
        state.devices.retain(|d| d.id != deviceID);
    }
}
