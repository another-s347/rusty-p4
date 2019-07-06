use crate::util::packet::Packet;
use bytes::Bytes;

#[derive(Clone)]
pub struct Data(pub Bytes);

impl Packet for Data {
    type Payload = ();

    fn from_bytes(b: Bytes) -> Option<Self> {
        Some(Data(b))
    }

    fn to_bytes(self) -> Bytes {
        self.0
    }
}