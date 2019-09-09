use byteorder::ByteOrder;
use bytes::{BufMut, Bytes, BytesMut};

use crate::util::packet::Packet;
use crate::util::value::MAC;
use nom::bytes::complete::take;
use nom::IResult;
use std::convert::TryFrom;
use std::fmt::Debug;
use std::fmt::Formatter;

pub struct Ethernet<'a, P> {
    pub src: &'a [u8; 6],
    pub dst: &'a [u8; 6],
    pub ether_type: u16,
    pub payload: P,
}

impl<'a, P> Debug for Ethernet<'a, P> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "\nsrc: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}\ndst: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}\ntype: {:x}",
            self.src[0], self.src[1], self.src[2], self.src[3], self.src[4], self.src[5],
            self.dst[0], self.dst[1], self.dst[2], self.dst[3], self.dst[4], self.dst[5],
            self.ether_type
        )
    }
}

impl<'a, P> Packet<'a> for Ethernet<'a, P>
where
    P: Packet<'a>,
{
    type Payload = P;

    fn self_bytes_hint(&self) -> usize {
        14
    }

    fn from_bytes(input: &'a [u8]) -> Option<Self> {
        let (b, dst) = take_mac(input).ok()?;
        let (b, src) = take_mac(b).ok()?;
        let (b, ether_type) = nom::number::complete::be_u16::<()>(b).ok()?;
        let payload = P::from_bytes(b)?;
        Some(Ethernet {
            src,
            dst,
            ether_type,
            payload,
        })
    }

    fn write_self_to_buf<T: BufMut>(&self, buf: &mut T) {
        buf.put_slice(self.dst);
        buf.put_slice(self.src);
        buf.put_u16_be(self.ether_type);
    }

    fn get_payload(&self) -> Option<&Self::Payload> {
        Some(&self.payload)
    }
}

fn take_mac(input: &[u8]) -> IResult<&[u8], &[u8; 6]> {
    let (b, t) = take(6usize)(input)?;
    Ok((b, <&[u8; 6]>::try_from(t).unwrap()))
}
