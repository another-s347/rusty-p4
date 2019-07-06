use crate::util::packet::Packet;

pub struct Data {

}

impl Packet for Data {
    type Payload = ();

    fn from_bytes(b: Vec<u8>) -> Option<Self> {
        unimplemented!()
    }

    fn to_bytes(&self) -> Vec<u8> {
        unimplemented!()
    }
}