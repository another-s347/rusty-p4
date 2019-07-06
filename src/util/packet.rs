pub mod ethernet;
pub mod ip;
pub mod data;
pub use ethernet::Ethernet as Ethernet;

pub trait Packet {
    type Payload;

    fn from_bytes(b:Vec<u8>) -> Option<Self>;

    fn to_bytes(&self) -> Vec<u8>;
}

impl Packet for () {
    type Payload = ();

    fn from_bytes(b: Vec<u8>) -> Option<Self> {
        Some(())
    }

    fn to_bytes(&self) -> Vec<u8> {
        Vec::new()
    }
}