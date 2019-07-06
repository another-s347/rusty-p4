use crate::util::packet::Packet;

pub struct Ethernet<P>
    where P:Packet
{

}

impl<P> Packet for Ethernet<P>
{
    type Payload = P;

    fn from_bytes(b: Vec<u8>) -> Option<Self> {
        unimplemented!()
    }

    fn to_bytes(&self) -> Vec<u8> {
        unimplemented!()
    }
}