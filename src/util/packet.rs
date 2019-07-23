use bytes::{Bytes, BytesMut};

pub use arp::Arp;
pub use ethernet::Ethernet;

pub mod arp;
pub mod data;
pub mod ethernet;
pub mod ip;

pub trait Packet
where
    Self: std::marker::Sized,
{
    type Payload;

    fn from_bytes(b: BytesMut) -> Option<Self>;

    fn into_bytes(self) -> Bytes;
}

impl Packet for () {
    type Payload = ();

    fn from_bytes(b: BytesMut) -> Option<Self> {
        Some(())
    }

    fn into_bytes(self) -> Bytes {
        Bytes::new()
    }
}
