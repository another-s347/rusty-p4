use crate::util::packet::Packet;
use bytes::{Bytes, BytesMut};

#[derive(Clone)]
pub struct Data(pub Bytes);

impl Packet for Data {
    type Payload = ();

    fn from_bytes(b: BytesMut) -> Option<Self> {
        Some(Data(b.freeze()))
    }

    fn into_bytes(self) -> Bytes {
        self.0
    }
}