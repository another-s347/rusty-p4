use bytes::{Bytes, BytesMut};

use crate::util::packet::{Packet, PacketRef};

#[derive(Clone, Debug)]
pub struct Data(pub Bytes);

#[derive(Clone, Debug)]
pub struct DataRef<'a> {
    pub inner: &'a [u8],
}

impl Packet for Data {
    type Payload = ();

    fn from_bytes(b: BytesMut) -> Option<Self> {
        Some(Data(b.freeze()))
    }

    fn into_bytes(self) -> Bytes {
        self.0
    }
}

impl<'a> PacketRef<'a> for DataRef<'a> {
    type Payload = ();

    fn from_bytes(b: &'a [u8]) -> Option<Self> {
        Some(DataRef { inner: b })
    }

    fn into_bytes(self) -> &'a [u8] {
        self.inner
    }
}

impl<'a> DataRef<'a> {
    pub fn to_data(&self) -> Data {
        Data {
            0: bytes::Bytes::from(self.inner),
        }
    }
}
