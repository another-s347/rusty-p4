use bytes::{Bytes, BytesMut};

pub use ethernet::Ethernet as Ethernet;
pub use arp::Arp as Arp;

pub mod ethernet;
pub mod ip;
pub mod arp;
pub mod data;
pub mod packet_in_header;

pub trait Packet
    where Self: std::marker::Sized
{
    type Payload;

    fn from_bytes(b:BytesMut) -> Option<Self>;

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