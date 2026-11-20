use core::mem::offset_of;
use core::mem::size_of;

use ftl_types::nic::IP_PROTO_UDP;

use crate::endian::read_u16_be;
use crate::endian::read_u32_be;
use crate::net::ipv4::Endpoint;
use crate::net::ipv4::IPV4_HEADER_LEN;
use crate::net::ipv4::Ipv4Header;

#[repr(C, packed)]
struct UdpHeader {
    src_port: u16,
    dst_port: u16,
    len: u16,
    checksum: u16,
}

const UDP_HEADER_LEN: usize = size_of::<UdpHeader>();

pub struct UdpPacketInfo {
    pub remote: Endpoint,
    pub local_port: u16,
    pub payload_len: usize,
}

impl UdpPacketInfo {
    pub fn parse(header: &[u8]) -> Self {
        let version_ihl = header[offset_of!(Ipv4Header, version_ihl)];
        let ip_header_len = (version_ihl & 0x0f) as usize * 4;
        let remote_ip = read_u32_be(header, offset_of!(Ipv4Header, src_ip));

        let udp_header = &header[ip_header_len..ip_header_len + UDP_HEADER_LEN];
        let local_port = read_u16_be(udp_header, offset_of!(UdpHeader, dst_port));
        let remote_port = read_u16_be(udp_header, offset_of!(UdpHeader, src_port));
        let len = read_u16_be(udp_header, offset_of!(UdpHeader, len)) as usize;

        UdpPacketInfo {
            remote: Endpoint {
                ip: remote_ip,
                port: remote_port,
            },
            local_port,
            payload_len: len - UDP_HEADER_LEN,
        }
    }
}

pub struct HeaderBuilder {
    header: [u8; IPV4_HEADER_LEN + UDP_HEADER_LEN],
}

impl HeaderBuilder {
    pub const fn new() -> Self {
        Self {
            header: [0u8; IPV4_HEADER_LEN + UDP_HEADER_LEN],
        }
    }

    pub fn build(
        mut self,
        remote: Endpoint,
        local_port: u16,
        payload_len: usize,
    ) -> [u8; IPV4_HEADER_LEN + UDP_HEADER_LEN] {
        self.write_ipv4_header(remote, payload_len);
        self.write_udp_header(remote, local_port, payload_len);
        self.header
    }

    fn write_ipv4_header(&mut self, remote: Endpoint, payload_len: usize) {
        let total_len = IPV4_HEADER_LEN + UDP_HEADER_LEN + payload_len;
        self.write_u8(offset_of!(Ipv4Header, version_ihl), 0x45);
        self.write_u8(offset_of!(Ipv4Header, ttl), 64);
        self.write_u8(offset_of!(Ipv4Header, protocol), IP_PROTO_UDP);
        self.write_u16(offset_of!(Ipv4Header, total_len), total_len as u16);
        self.write_u16(offset_of!(Ipv4Header, flags_and_frag), 0x4000);
        self.write_u32(offset_of!(Ipv4Header, src_ip), 0);
        self.write_u32(offset_of!(Ipv4Header, dst_ip), remote.ip);
    }

    fn write_udp_header(&mut self, remote: Endpoint, local_port: u16, payload_len: usize) {
        let base = IPV4_HEADER_LEN;

        let len = UDP_HEADER_LEN + payload_len;
        self.write_u16(base + offset_of!(UdpHeader, src_port), local_port);
        self.write_u16(base + offset_of!(UdpHeader, dst_port), remote.port);
        self.write_u16(base + offset_of!(UdpHeader, len), len as u16);
    }

    fn write_u8(&mut self, offset: usize, value: u8) {
        self.header[offset] = value;
    }

    fn write_u16(&mut self, offset: usize, value: u16) {
        self.header[offset..offset + size_of::<u16>()].copy_from_slice(&value.to_be_bytes());
    }

    fn write_u32(&mut self, offset: usize, value: u32) {
        self.header[offset..offset + size_of::<u32>()].copy_from_slice(&value.to_be_bytes());
    }
}
