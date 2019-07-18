use super::{linkprobe, proxyarp};
use crate::app::common::{CommonState, MergeResult};
use crate::app::p4App;
use crate::context::ContextHandle;
use crate::event::{CommonEvents, Event, PacketReceived};
use crate::representation::{Device, Host, Link};
use crate::util::packet::arp;
use crate::util::packet::data::Data;
use crate::util::packet::Ethernet;
use crate::util::packet::Packet;
use bytes::BytesMut;
use log::{debug, error, info, trace, warn};
use serde::export::PhantomData;

pub trait p4AppExtended<E> {
    fn on_packet(
        self: &mut Self,
        packet: PacketReceived,
        ctx: &ContextHandle<E>,
        state: &CommonState,
    ) {
    }
    fn on_event(self: &mut Self, event: E, ctx: &ContextHandle<E>) {}
    fn on_host_added(self: &mut Self, host: &Host, ctx: &ContextHandle<E>) {}
    fn on_device_added(self: &mut Self, device: &Device, ctx: &ContextHandle<E>) {}
    fn on_link_added(self: &mut Self, link: &Link, ctx: &ContextHandle<E>) {}
}

pub struct p4AppExtendedCore<A, E> {
    common: CommonState,
    extension: A,
    phantom: PhantomData<E>,
}

impl<A, E> p4App<E> for p4AppExtendedCore<A, E>
where
    A: p4AppExtended<E>,
    E: Event,
{
    fn on_packet(self: &mut Self, packet: PacketReceived, ctx: &ContextHandle<E>) {
        let bytes = BytesMut::from(packet.packet.payload.clone());
        let device = self.common.devices.get(&packet.from);
        if let Some(device) = device {
            let ethernet: Option<Ethernet<Data>> = Ethernet::from_bytes(bytes);
            if let Some(eth) = ethernet {
                match eth.ether_type {
                    0x861 => {
                        linkprobe::on_probe_received(device, packet.port, eth.payload, ctx);
                    }
                    arp::ETHERNET_TYPE_ARP => proxyarp::on_arp_received(
                        device,
                        packet.port,
                        eth.payload,
                        &self.common,
                        ctx,
                    ),
                    _ => {
                        self.extension.on_packet(packet, ctx, &self.common);
                    }
                }
            }
        } else {
            error!(target:"extend","device not found with name: {}", packet.from);
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
                        self.extension.on_device_added(&device, ctx);
                    }
                    _ => unimplemented!(),
                }
            }
            CommonEvents::HostDetected(host) => {
                let result = self.common.merge_host(host);
                match result {
                    MergeResult::ADDED(host) => {
                        info!(target:"extend","host detected {:?}",host);
                        self.extension.on_host_added(&host, ctx);
                    }
                    MergeResult::CONFLICT => {}
                    MergeResult::MERGED => {}
                }
            }
            CommonEvents::LinkDetected(link) => {
                let result = self.common.add_link(link.clone(), 1);
                match result {
                    MergeResult::ADDED(()) => {
                        self.extension.on_link_added(&link, ctx);
                    }
                    MergeResult::CONFLICT => {}
                    MergeResult::MERGED => {}
                }
            }
            _ => {}
        };
        self.extension.on_event(event, ctx);
    }
}

pub struct ExampleExtended {}

impl p4AppExtended<CommonEvents> for ExampleExtended {}

pub fn extend<E: Event, A: p4AppExtended<E>>(app: A) -> p4AppExtendedCore<A, E> {
    p4AppExtendedCore {
        common: CommonState::new(),
        extension: app,
        phantom: PhantomData,
    }
}
