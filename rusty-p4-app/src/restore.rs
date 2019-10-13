use rusty_p4::app::P4app;
use rusty_p4::context::{Context, ContextHandle};
use rusty_p4::event::CommonEvents;
use rusty_p4::event::{CoreRequest, Event, PacketReceived};
use rusty_p4::representation::DeviceType;
use rusty_p4::representation::{Device, DeviceID};
use serde::{Deserialize, Serialize};
use serde_json::*;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::{Arc, Mutex};
use async_trait::async_trait;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RestoreState {
    pub devices: HashMap<DeviceID, Device>,
}

impl RestoreState {
    pub fn new() -> RestoreState {
        RestoreState {
            devices: HashMap::new(),
        }
    }
}

#[async_trait]
impl<E> P4app<E> for Restore
where
    E: Event,
{
    async fn on_start(self: &mut Self, ctx: &mut ContextHandle<E>) {
        self.file.seek(SeekFrom::Start(0));
        self.file.set_len(0);
        serde_json::to_writer_pretty(&mut self.file, &self.state).unwrap();
        self.file.flush().unwrap();
        for (_, device) in self.state.devices.drain() {
            match device.typ {
                DeviceType::MASTER {
                    socket_addr,
                    device_id,
                    pipeconf,
                } => ctx.add_device_with_pipeconf_id(device.name, socket_addr, device_id, pipeconf),
                _ => {}
            }
        }
    }

    async fn on_event(&mut self, event: E, ctx: &mut ContextHandle<E>) -> Option<E> {
        match event.try_to_common() {
            Some(CommonEvents::DeviceAdded(device)) if device.typ.is_master() => {
                if !self.state.devices.contains_key(&device.id) {
                    self.state.devices.insert(device.id, device.clone());
                    self.file.seek(SeekFrom::Start(0));
                    self.file.set_len(0);
                    serde_json::to_writer_pretty(&mut self.file, &self.state).unwrap();
                    self.file.flush().unwrap();
                }
            }
            Some(CommonEvents::DeviceLost(device)) => {
                if self.state.devices.contains_key(device) {
                    self.state.devices.remove(device);
                    self.file.seek(SeekFrom::Start(0));
                    self.file.set_len(0);
                    println!("write lost");
                    serde_json::to_writer_pretty(&mut self.file, &self.state).unwrap();
                    self.file.flush().unwrap();
                }
            }
            _ => {}
        };
        Some(event)
    }
}

pub struct Restore {
    pub state: RestoreState,
    pub file: std::fs::File,
}

impl Restore {
    pub fn new<T: AsRef<Path>>(path: T) -> Restore {
        if let Ok(file_existed) = OpenOptions::new()
            .write(true)
            .read(true)
            .open(path.as_ref())
        {
            if let Ok(state) = serde_json::from_reader(&file_existed) {
                Restore {
                    state,
                    file: file_existed,
                }
            } else {
                panic!("file existed with bad record")
            }
        } else {
            Restore {
                state: RestoreState::new(),
                file: File::create(path).unwrap(),
            }
        }
    }
}
