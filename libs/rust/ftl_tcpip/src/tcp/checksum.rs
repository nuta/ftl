use crate::checksum::Checksum;
use crate::ip::ipv4::Ipv4Addr;
use crate::tcp::header::TcpHeader;
use crate::transport::Protocol;

pub(super) fn compute_checksum(
    header: &TcpHeader,
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    payload: &[u8],
) -> u16 {
    let tcp_len = size_of::<TcpHeader>() + payload.len();
    debug_assert!(tcp_len <= u16::MAX as usize);

    let mut checksum = Checksum::new();
    checksum.supply_u32(src_ip.as_u32());
    checksum.supply_u32(dst_ip.as_u32());
    checksum.supply_u16(Protocol::Tcp as u16);
    checksum.supply_u16(tcp_len as u16);
    checksum.supply_u16(header.src_port.into());
    checksum.supply_u16(header.dst_port.into());
    checksum.supply_u32(header.seq.into());
    checksum.supply_u32(header.ack.into());
    checksum.supply_u16(((header.header_len as u16) << 8) | header.flags.as_u8() as u16);
    checksum.supply_u16(header.window_size.into());
    checksum.supply_u16(0);
    checksum.supply_u16(header.urgent_pointer.into());
    checksum.supply_bytes(payload);
    checksum.finish()
}
