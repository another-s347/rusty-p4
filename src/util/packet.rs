pub mod ethernet;
pub mod ip;
pub mod data;
pub use ethernet::Ethernet as Ethernet;
pub mod packet_in_header;

use bytes::Bytes;

pub trait Packet
    where Self: std::marker::Sized
{
    type Payload;

    fn from_bytes(b:Bytes) -> Option<Self>;

    fn into_bytes(self) -> Bytes;
}

impl Packet for () {
    type Payload = ();

    fn from_bytes(b: Bytes) -> Option<Self> {
        Some(())
    }

    fn into_bytes(self) -> Bytes {
        Bytes::new()
    }
}