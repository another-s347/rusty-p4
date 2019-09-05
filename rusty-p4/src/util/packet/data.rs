use bytes::{BufMut, Bytes, BytesMut};

use crate::util::packet::{Packet, PacketRef};

#[derive(Clone, Debug)]
pub struct Data(pub Bytes);

#[derive(Clone, Debug)]
pub struct DataRef<'a> {
    pub inner: &'a [u8],
}

impl Packet for Data {
    type Payload = ();

    fn bytes_hint(&self) -> usize {
        self.0.len()
    }

    fn from_bytes(b: BytesMut) -> Option<Self> {
        Some(Data(b.freeze()))
    }

    fn into_bytes(self) -> Bytes {
        self.0
    }
}

impl<'a> PacketRef<'a> for &'a [u8] {
    type Payload = ();

    fn self_bytes_hint(&self) -> usize {
        self.len()
    }

    fn from_bytes(b: &'a [u8]) -> Option<Self> {
        Some(b)
    }

    fn write_self_to_buf<T: BufMut>(&self, mut buf: T) {
        buf.put_slice(self)
    }

    fn get_payload(&self) -> Option<&Self::Payload> {
        None
    }
}
