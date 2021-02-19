use bytes::{BufMut, Bytes, BytesMut};

pub use arp::Arp;
pub use ethernet::Ethernet;

pub mod arp;
pub mod data;
pub mod ethernet;
pub mod ipv4;

pub trait Packet<'a>
where
    Self: Sized,
{
    type Payload: Packet<'a>;

    fn self_bytes_hint(&self) -> usize;

    fn from_bytes(b: &'a [u8]) -> Option<Self>;

    fn write_self_to_buf<T: BufMut>(&self, buf: &mut T);

    fn write_all_to_buf<T: BufMut>(&self, buf: &mut T) {
        self.write_self_to_buf(buf);
        if let Some(payload) = self.get_payload() {
            payload.write_all_to_buf(buf);
        }
    }

    fn get_payload(&self) -> Option<&Self::Payload>;

    fn bytes_hint(&self) -> usize {
        let packet = self;
        let mut size = 0;
        size += packet.self_bytes_hint();
        if let Some(payload) = packet.get_payload() {
            size += payload.bytes_hint();
        }
        size
    }

    fn write_to_bytes(&self) -> Bytes {
        let packet = self;
        let mut buffer = BytesMut::with_capacity(packet.bytes_hint());
        self.write_all_to_buf(&mut buffer);
        buffer.freeze()
    }
}

impl<'a> Packet<'a> for () {
    type Payload = ();

    fn self_bytes_hint(&self) -> usize {
        0
    }

    fn from_bytes(_b: &'a [u8]) -> Option<Self> {
        Some(())
    }

    fn write_self_to_buf<T: BufMut>(&self, _buf: &mut T) {}

    fn get_payload(&self) -> Option<&Self::Payload> {
        None
    }
}
