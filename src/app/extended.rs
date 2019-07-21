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

pub type EventHook<E> = Box<FnMut(&E, &CommonState, &ContextHandle<E>) -> ()>;
pub type EtherPacketHook<E> = Box<FnMut(Data, ConnectPoint, &CommonState, &ContextHandle<E>) -> ()>;

pub trait P4appExtended<E> {
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
            },
        }
    }

    pub fn with(&mut self, mut i: impl P4appInstallable<A, E>) -> &mut Self {
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
                    .find(|(k, _)| k == &&eth.ether_type)
                {
                    h(eth.payload, packet.from, state, ctx);
                }
                //                match eth.ether_type {
                //                    0x861 => {
                //                        linkprobe::on_probe_received(device, packet.from, eth.payload, ctx);
                //                    }
                //                    arp::ETHERNET_TYPE_ARP => proxyarp::on_arp_received(
                //                        device,
                //                        packet.from,
                //                        eth.payload,
                //                        &self.common,
                //                        ctx,
                //                    ),
                //                    _ => {
                //                        self.extension.on_packet(packet, ctx, &self.common);
                //                    }
                //                }
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
                        linkprobe::on_device_added(&device, ctx);
                        proxyarp::on_device_added(&device, ctx);
                        self.extension.on_device_added(&device, &self.common, ctx);
                    }
                    _ => unimplemented!(),
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
            return None;
        } else {
            self.event_hooks.insert(name.to_owned(), hook);
            return Some(());
        }
    }

    pub fn install_ether_hook(&mut self, ether_type: u16, hook: EtherPacketHook<E>) -> Option<()> {
        if self.ether_hooks.contains_key(&ether_type) {
            return None;
        } else {
            self.ether_hooks.insert(ether_type, hook);
            return Some(());
        }
    }
}

pub struct ExampleExtended {}

impl P4appExtended<CommonEvents> for ExampleExtended {}

pub fn extend<E: Event, A: P4appExtended<E>>(app: A) -> P4appExtendedCore<A, E> {
    P4appExtendedCore {
        common: CommonState::new(),
        extension: app,
        phantom: PhantomData,
        event_hooks: HashMap::new(),
        ether_hooks: HashMap::new(),
    }
}
