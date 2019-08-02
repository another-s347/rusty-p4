use super::{linkprobe, proxyarp};
use crate::app::common::{CommonState, MergeResult};
use crate::app::P4app;
use crate::context::ContextHandle;
use crate::error::*;
use crate::event::{CommonEvents, Event, PacketReceived};
use crate::representation::{ConnectPoint, Device, Host, Link};
use crate::util::packet::arp;
use crate::util::packet::data::Data;
use crate::util::packet::Ethernet;
use crate::util::packet::Packet;
use bytes::BytesMut;
use log::{debug, error, info, trace, warn};
use serde::export::PhantomData;
use std::collections::HashMap;

pub type EventHook<E> = Box<dyn FnMut(&E, &CommonState, &ContextHandle<E>) -> () + Send>;
pub type EtherPacketHook<E> =
    Box<dyn FnMut(Ethernet<Data>, ConnectPoint, &CommonState, &ContextHandle<E>) -> () + Send>;
pub type OnDeviceAddedHook<E> =
    Box<dyn FnMut(&Device, &CommonState, &ContextHandle<E>) -> () + Send>;

pub trait P4appExtended<E>: Send + 'static {
    fn on_packet(
        self: &mut Self,
        packet: PacketReceived,
        ctx: &ContextHandle<E>,
        state: &CommonState,
    ) {
    }
    fn on_event(self: &mut Self, event: E, state: &CommonState, ctx: &ContextHandle<E>) {}
    fn on_host_added(self: &mut Self, host: &Host, state: &CommonState, ctx: &ContextHandle<E>) {}
    fn on_device_added(
        self: &mut Self,
        device: &Device,
        state: &CommonState,
        ctx: &ContextHandle<E>,
    ) {
    }
    fn on_link_added(self: &mut Self, link: &Link, state: &CommonState, ctx: &ContextHandle<E>) {}
}

pub trait P4appInstallable<A, E> {
    fn install(&mut self, extend_core: &mut P4appExtendedCore<A, E>);
}

pub struct P4appExtendedCore<A, E> {
    common: CommonState,
    extension: A,
    phantom: PhantomData<E>,
    event_hooks: HashMap<String, EventHook<E>>,
    ether_hooks: HashMap<u16, EtherPacketHook<E>>,
    device_added_hooks: HashMap<String, OnDeviceAddedHook<E>>,
}

pub struct P4appBuilder<A, E> {
    core: P4appExtendedCore<A, E>,
}

impl<A, E> P4appBuilder<A, E>
where
    A: P4appExtended<E>,
    E: Event,
{
    pub fn new(app: A) -> Self {
        P4appBuilder {
            core: P4appExtendedCore {
                common: CommonState::new(),
                extension: app,
                phantom: PhantomData,
                event_hooks: HashMap::new(),
                ether_hooks: HashMap::new(),
                device_added_hooks: HashMap::new(),
            },
        }
    }

    pub fn with(mut self, mut i: impl P4appInstallable<A, E>) -> Self {
        i.install(&mut self.core);
        self
    }

    pub fn build(self) -> P4appExtendedCore<A, E> {
        self.core
    }
}

impl<A, E> P4app<E> for P4appExtendedCore<A, E>
where
    A: P4appExtended<E>,
    E: Event,
{
    fn on_packet(self: &mut Self, packet: PacketReceived, ctx: &ContextHandle<E>) {
        let bytes = BytesMut::from(packet.packet.payload.clone());
        let device = self.common.devices.get(&packet.from.device);
        let state = &self.common;
        if let Some(device) = device {
            let ethernet: Option<Ethernet<Data>> = Ethernet::from_bytes(bytes);
            if let Some(eth) = ethernet {
                if let Some((_, h)) = self
                    .ether_hooks
                    .iter_mut()
                    .find(|(&k, _)| k == eth.ether_type)
                {
                    h(eth, packet.from, state, ctx);
                }
                self.extension.on_packet(packet, ctx, &self.common);
            }
        } else {
            error!(target:"extend","device not found with id: {:?}", packet.from.device);
        }
    }

    fn on_event(self: &mut Self, event: E, ctx: &ContextHandle<E>) {
        let common: CommonEvents = event.clone().into();
        match common {
            CommonEvents::DeviceAdded(device) => {
                let result = self.common.merge_device(device);
                match result {
                    MergeResult::ADDED(name) => {
                        let device = self.common.devices.get(&name).unwrap();
                        let state = &self.common;
                        self.device_added_hooks.iter_mut().for_each(|(h_name, h)| {
                            debug!(target:"extend","executing device added hook: {}",h_name);
                            h(&device, &state, ctx);
                        });
                        self.extension.on_device_added(&device, &self.common, ctx);
                    }
                    MergeResult::MERGED => {}
                    MergeResult::CONFLICT => unimplemented!(),
                }
            }
            CommonEvents::HostDetected(host) => {
                let result = self.common.merge_host(host);
                match result {
                    MergeResult::ADDED(host) => {
                        info!(target:"extend","host detected {:?}",host);
                        self.extension.on_host_added(&host, &self.common, ctx);
                    }
                    MergeResult::CONFLICT => {}
                    MergeResult::MERGED => {}
                }
            }
            CommonEvents::LinkDetected(link) => {
                let result = self.common.add_link(link.clone(), 1);
                match result {
                    MergeResult::ADDED(()) => {
                        self.extension.on_link_added(&link, &self.common, ctx);
                    }
                    MergeResult::CONFLICT => {}
                    MergeResult::MERGED => {}
                }
            }
            CommonEvents::DeviceLost(deviceID) => {
                if let Some(device) = self.common.devices.remove(&deviceID) {
                    self.common
                        .hosts
                        .iter()
                        .filter(|host| host.location.device == deviceID)
                        .for_each(|host| {
                            ctx.send_event(CommonEvents::HostLost(*host));
                        });
                    self.common
                        .links
                        .iter()
                        .filter(|x| x.src.device == deviceID || x.dst.device == deviceID)
                        .for_each(|link| {
                            ctx.send_event(CommonEvents::LinkLost(*link));
                        });
                    self.common.graph.remove_device(&deviceID);
                    info!(target:"extend","device removed {}", device.name);
                } else {
                    warn!(target:"extend","duplicated device lost event {:?}", deviceID);
                }
            }
            CommonEvents::LinkLost(link) => {
                if self.common.links.remove(&link) {
                    self.common.graph.remove_link(&link);
                    info!(target:"extend","link removed {:?}", link);
                } else {
                    warn!(target:"extend","duplicated link lost event {:?}", link);
                }
            }
            CommonEvents::HostLost(host) => {
                if self.common.hosts.remove(&host) {
                    info!(target:"extend","host removed {:?}", &host);
                } else {
                    warn!(target:"extend","duplicated host lost event {:?}", &host);
                }
            }
            _ => {}
        };
        let state = &self.common;
        self.event_hooks.iter_mut().for_each(|(h_name, h)| {
            h(&event, state, ctx);
        });
        self.extension.on_event(event, &self.common, ctx);
    }
}

impl<A, E> P4appExtendedCore<A, E> {
    pub fn install_event_hook(&mut self, name: &str, hook: EventHook<E>) -> Option<()> {
        if self.event_hooks.contains_key(name) {
            error!(target:"extend","install event hook fail for: {}",name);
            return None;
        } else {
            self.event_hooks.insert(name.to_owned(), hook);
            return Some(());
        }
    }

    pub fn install_ether_hook(&mut self, ether_type: u16, hook: EtherPacketHook<E>) -> Option<()> {
        if self.ether_hooks.contains_key(&ether_type) {
            error!(target:"extend","install ether hook fail for: {}",ether_type);
            return None;
        } else {
            self.ether_hooks.insert(ether_type, hook);
            return Some(());
        }
    }

    pub fn install_device_added_hook(
        &mut self,
        name: &str,
        hook: OnDeviceAddedHook<E>,
    ) -> Option<()> {
        if self.device_added_hooks.contains_key(name) {
            return None;
        } else {
            self.device_added_hooks.insert(name.to_owned(), hook);
            return Some(());
        }
    }
}

pub struct ExampleExtended {}

impl P4appExtended<CommonEvents> for ExampleExtended {}
