use super::Packet;
use bytes::BufMut;
use nom::bytes::complete::take;

#[derive(Debug)]
pub struct Ipv4<'a, P> {
    version: u8,
    ihl: u8,
    dscp: u8,
    ecn: u8,
    total_len: u16,
    identification: u16,
    flags: u8,
    frag_offset: u16,
    ttl: u8,
    protocol: u8,
    hdr_checksum: u16,
    src: &'a [u8],
    dst: &'a [u8],
    payload: P,
}
//fn t() {
//    let data = vec![
//        0x45, 0x00, 0x00, 0x40, 0x69, 0x27, 0x40, 0x00, 0x40, 0x11, 0x4d, 0x0d, 0xc0, 0xa8, 0x01,
//        0x2a, 0xc0, 0xa8, 0x01, 0xfe,
//    ];
//    let bytes = BytesMut::from(data);
//
//    let header = Ipv4::<()>::from_bytes(bytes).unwrap();
//
//    println!("{:#?}", header);
//}
impl<'a, P> Packet<'a> for Ipv4<'a, P>
where
    P: Packet<'a>,
{
    type Payload = P;

    fn self_bytes_hint(&self) -> usize {
        20
    }

    fn from_bytes(b: &'a [u8]) -> Option<Self> {
        let (b, version_ihl) = nom::number::complete::be_u8::<()>(b).ok()?;
        let version = (version_ihl & 0b11110000) >> 4;
        let ihl = version_ihl & 0b00001111;
        if ihl > 5 {
            return None;
        }
        let (b, dscp_ecn) = nom::number::complete::be_u8::<()>(b).ok()?;
        let dscp = (dscp_ecn & 0b11111100) >> 2;
        let ecn = dscp_ecn & 0b00000011;
        let (b, total_len) = nom::number::complete::be_u16::<()>(b).ok()?;
        let (b, identification) = nom::number::complete::be_u16::<()>(b).ok()?;
        let (b, flags_fragmentoffset) = nom::number::complete::be_u16::<()>(b).ok()?;
        let flags = ((flags_fragmentoffset & 0b1110_0000_0000_0000) >> 13) as u8;
        let frag_offset = flags_fragmentoffset & 0b0001_1111_1111_1111;
        let (b, ttl) = nom::number::complete::be_u8::<()>(b).ok()?;
        let (b, protocol) = nom::number::complete::be_u8::<()>(b).ok()?;
        let (b, hdr_checksum) = nom::number::complete::be_u16::<()>(b).ok()?;
        let (b, src_ip) = take::<_, _, ()>(4u8)(b).ok()?;
        let (b, dst_ip) = take::<_, _, ()>(4u8)(b).ok()?;
        let payload = P::from_bytes(b)?;
        Some(Self {
            version,
            ihl,
            dscp,
            ecn,
            total_len,
            identification,
            flags,
            frag_offset,
            ttl,
            protocol,
            hdr_checksum,
            src: src_ip,
            dst: dst_ip,
            payload,
        })
    }

    fn write_self_to_buf<T: BufMut>(&self, buf: &mut T) {
        let version_ihl = (self.version << 4) + (self.ihl & 0b00001111);
        buf.put_u8(version_ihl);
        let dscp_ecn = (self.dscp << 2) + (self.ecn & 0b00000011);
        buf.put_u8(dscp_ecn);
        buf.put_u16(self.total_len);
        buf.put_u16(self.identification);
        let flags_fragmentoffset =
            ((self.flags as u16) << 13) + (self.frag_offset & 0b0001_1111_1111_1111);
        buf.put_u16(flags_fragmentoffset);
        buf.put_u8(self.ttl);
        buf.put_u8(self.protocol);
        buf.put_u16(self.hdr_checksum);
        buf.put_slice(self.src);
        buf.put_slice(self.dst);
    }

    fn get_payload(&self) -> Option<&Self::Payload> {
        Some(&self.payload)
    }
}
