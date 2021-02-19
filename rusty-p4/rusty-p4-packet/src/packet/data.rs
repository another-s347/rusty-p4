use bytes::BufMut;

use super::Packet;

impl<'a> Packet<'a> for &'a [u8] {
    type Payload = ();

    fn self_bytes_hint(&self) -> usize {
        self.len()
    }

    fn from_bytes(b: &'a [u8]) -> Option<Self> {
        Some(b)
    }

    fn write_self_to_buf<T: BufMut>(&self, buf: &mut T) {
        buf.put_slice(self)
    }

    fn get_payload(&self) -> Option<&Self::Payload> {
        None
    }
}
