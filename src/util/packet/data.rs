use bytes::{Bytes, BytesMut};

use crate::util::packet::Packet;

#[derive(Clone,Debug)]
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