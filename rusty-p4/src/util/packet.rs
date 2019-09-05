use bytes::{BufMut, Bytes, BytesMut};

pub use arp::Arp;
pub use ethernet::Ethernet;
use failure::_core::marker::PhantomData;

pub mod arp;
pub mod data;
pub mod ethernet;
pub mod ip;

pub trait Packet<'a>
where
    Self: Sized,
{
    type Payload: Packet<'a>;

    fn self_bytes_hint(&self) -> usize;

    fn from_bytes(b: &'a [u8]) -> Option<Self>;

    fn write_self_to_buf<T: BufMut>(&self, mut buf: T);

    fn get_payload(&self) -> Option<&Self::Payload>;

    fn bytes_hint(&self) -> usize {
        let packet = self;
        let mut size = 0;
        size += packet.self_bytes_hint();
        while let Some(packet) = packet.get_payload() {
            size += packet.bytes_hint();
        }
        size
    }

    fn write_to_bytes(&self) -> Bytes {
        let packet = self;
        let mut buffer = BytesMut::with_capacity(packet.bytes_hint());
        packet.write_self_to_buf(&mut buffer);
        while let Some(packet) = packet.get_payload() {
            packet.write_self_to_buf(&mut buffer);
        }
        buffer.freeze()
    }
}

impl<'a> Packet<'a> for () {
    type Payload = ();

    fn self_bytes_hint(&self) -> usize {
        0
    }

    fn from_bytes(b: &'a [u8]) -> Option<Self> {
        Some(())
    }

    fn write_self_to_buf<T: BufMut>(&self, mut buf: T) {}

    fn get_payload(&self) -> Option<&Self::Payload> {
        None
    }
}
