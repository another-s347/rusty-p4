pub mod ethernet;
pub mod ip;
pub mod data;
pub use ethernet::Ethernet as Ethernet;
pub mod packet_in_header;

use bytes::{BytesMut, Bytes};

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