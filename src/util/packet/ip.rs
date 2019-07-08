use std::net::{Ipv4Addr, Ipv6Addr};
use crate::util::packet::Packet;
use bytes::{Bytes, BytesMut};
use bitfield;

#[derive(Debug)]
pub struct Ipv4<P>
    where P:Packet
{
    version: u8,
    ihl:u8,
    dscp: u8,
    ecn: u8,
    total_len: u16,
    identification: u32,
    has_dont_fragment: bool,
    has_more_fragments: bool,
    frag_offset: u16,
    ttl: u8,
    protocol: u8,
    hdr_checksum: u32,
    src:Ipv4Addr,
    dst:Ipv4Addr,
    payload:P
}

bitfield! {
    pub struct Ipv4Raw(MSB0 [u8]);
    impl Debug;
    u32;
    u8, get_version, _: 3, 0;
    u8, get_ihl, _: 7, 4;
    u8, get_dscp, _: 13, 8;
    u8, get_ecn, _: 15, 14;
    u16, get_total_length, _: 31, 16;
    get_identification, _: 47, 31;
    get_df, _: 49;
    get_mf, _: 50;
    u16, get_fragment_offset, _: 63, 51;
    u8, get_time_to_live, _: 71, 64;
    u8, get_protocol, _: 79, 72;
    get_header_checksum, _: 95, 79;
    u8, get_source_address, _: 103, 96, 4;
    u8, get_destination_address, _: 135, 128, 4;
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> Ipv4Raw<T> {
    fn get_source_as_ip_addr(&self) -> Ipv4Addr {
        let mut src = [0; 4];
        for (i, src) in src.iter_mut().enumerate() {
            *src = self.get_source_address(i);
        }
        src.into()
    }

    fn get_destination_as_ip_addr(&self) -> Ipv4Addr {
        let mut src = [0; 4];
        for (i, src) in src.iter_mut().enumerate() {
            *src = self.get_destination_address(i);
        }
        src.into()
    }
}

impl<P> Packet for Ipv4<P>
    where P:Packet
{
    type Payload = P;

    fn from_bytes(mut b: BytesMut) -> Option<Self> {
        if b.len() < 20 {
            return None;
        }
        let x = b.split_to(20);
        let raw = Ipv4Raw(x);
        let payload = P::from_bytes(b);
        if payload.is_none() {
            return None;
        }
        let part = Ipv4 {
            version: raw.get_version(),
            ihl: raw.get_ihl(),
            dscp: raw.get_dscp(),
            ecn: raw.get_ecn(),
            total_len: raw.get_total_length(),
            identification: raw.get_identification(),
            has_dont_fragment: raw.get_df(),
            has_more_fragments: raw.get_mf(),
            frag_offset: raw.get_fragment_offset(),
            ttl: raw.get_time_to_live(),
            protocol: raw.get_protocol(),
            hdr_checksum: raw.get_header_checksum(),
            src: raw.get_source_as_ip_addr(),
            dst: raw.get_destination_as_ip_addr(),
            payload: payload.unwrap()
        };
        Some(part)
    }

    fn into_bytes(self) -> Bytes {
        unimplemented!()
    }
}

fn t() {
    let data = vec![
        0x45, 0x00, 0x00, 0x40, 0x69, 0x27, 0x40, 0x00, 0x40, 0x11, 0x4d, 0x0d, 0xc0, 0xa8, 0x01,
        0x2a, 0xc0, 0xa8, 0x01, 0xfe,
    ];
    let bytes = BytesMut::from(data);

    let header = Ipv4::<()>::from_bytes(bytes).unwrap();

    println!("{:#?}", header);
}