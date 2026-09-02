use ftl_types::error::ErrorCode;

use super::device::Device;
use super::device::Tx;
use super::packet::ipv4::Ipv4Addr;
use super::packet::ipv4::Ipv4Rewriter;
use super::packet::udp::IPPROTO_UDP;
use super::packet::udp::UDP_HEADER_LEN;
use super::packet::udp::UdpRewriter;
use crate::net::GLOBAL_ENV;
use crate::shared_ref::SharedRef;

const IPV4_BROADCAST: Ipv4Addr = Ipv4Addr::new(u32::MAX);
const IPV4_UNSPECIFIED: Ipv4Addr = Ipv4Addr::new(0);

pub fn send_broadcast(
    device: &SharedRef<Device>,
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
) -> Result<(), ErrorCode> {
    let packet_len = Ipv4Rewriter::HEADER_LEN + UDP_HEADER_LEN + payload.len();
    let total_len = u16::try_from(packet_len).map_err(|_| ErrorCode::InvalidArg)?;
    let mut tx = Tx::alloc(&GLOBAL_ENV, packet_len, 0)?;
    let packet = tx.header_bytes();
    packet.fill(0);

    let udp_offset = Ipv4Rewriter::HEADER_LEN;
    let udp_packet = &mut packet[udp_offset..];
    udp_packet[UDP_HEADER_LEN..].copy_from_slice(payload);
    let mut udp = UdpRewriter::new(udp_packet).map_err(|_| ErrorCode::InvalidArg)?;
    udp.set_src_port(src_port);
    udp.set_dst_port(dst_port);
    udp.set_len();
    udp.update_checksum(IPV4_UNSPECIFIED, IPV4_BROADCAST);

    let mut ipv4 = Ipv4Rewriter::new(packet).map_err(|_| ErrorCode::InvalidArg)?;
    ipv4.set_version_and_header_len();
    ipv4.set_total_len(total_len);
    ipv4.set_identification(0);
    ipv4.set_flags_and_fragment_offset(0);
    ipv4.set_ttl(64);
    ipv4.set_ip_proto(IPPROTO_UDP);
    ipv4.set_src_ip(IPV4_UNSPECIFIED);
    ipv4.set_dst_ip(IPV4_BROADCAST);
    ipv4.update_checksum();

    device.send_ipv4_broadcast(&GLOBAL_ENV, tx)
}
