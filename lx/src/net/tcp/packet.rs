use core::mem::offset_of;
use core::mem::size_of;
use core::ops::BitOr;
use core::ops::BitOrAssign;

use ftl_types::net::ETHTYPE_IPV4;
use ftl_types::net::FiveTuple;
use ftl_types::net::IPPROTO_TCP;

#[repr(C, packed)]
struct Ipv4Header {
    version_ihl: u8,
    dscp_ecn: u8,
    total_len: u16,
    id: u16,
    flags_and_frag: u16,
    ttl: u8,
    protocol: u8,
    checksum: u16,
    src_ip: u32,
    dst_ip: u32,
}

const IPV4_HEADER_LEN: usize = size_of::<Ipv4Header>();

#[repr(C, packed)]
struct TcpHeader {
    src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
    data_offset: u8,
    flags: u8,
    window_size: u16,
    checksum: u16,
    urgent_pointer: u16,
}

const TCP_HEADER_LEN: usize = size_of::<TcpHeader>();

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Endpoint {
    pub ip: u32,
    pub port: u16,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TcpFlags(u8);

impl TcpFlags {
    pub const FIN: Self = Self(1 << 0);
    pub const SYN: Self = Self(1 << 1);
    pub const RST: Self = Self(1 << 2);
    pub const PSH: Self = Self(1 << 3);
    pub const ACK: Self = Self(1 << 4);

    pub const fn from_u8(value: u8) -> Self {
        Self(value)
    }

    pub const fn as_u8(self) -> u8 {
        self.0
    }

    pub const fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }
}

impl BitOr for TcpFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for TcpFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

pub struct Segment<'a> {
    pub seq: u32,
    pub ack: u32,
    pub window_size: u16,
    pub flags: TcpFlags,
    pub payload: &'a [u8],
}

pub struct TcpPacketInfo {
    pub remote_ip: u32,
    pub local_ip: u32,
    pub remote_port: u16,
    pub local_port: u16,
    pub seq: u32,
    pub ack: u32,
    pub payload_len: u16,
    pub window_size: u16,
    pub flags: u8,
}

impl TcpPacketInfo {
    pub fn parse(header: &[u8]) -> Self {
        // Parser IPv4 header. We can skip validation here because the kernel has
        // already done it.
        let version_ihl = header[offset_of!(Ipv4Header, version_ihl)];
        let ip_header_len = (version_ihl & 0x0f) as usize * 4;
        let remote_ip = read_u32(header, offset_of!(Ipv4Header, src_ip));
        let local_ip = read_u32(header, offset_of!(Ipv4Header, dst_ip));
        let total_len = read_u16(header, offset_of!(Ipv4Header, total_len)) as usize;

        // Parser TCP header.
        let tcp_header = &header[ip_header_len..ip_header_len + TCP_HEADER_LEN];
        let local_port = read_u16(tcp_header, offset_of!(TcpHeader, dst_port));
        let remote_port = read_u16(tcp_header, offset_of!(TcpHeader, src_port));
        let seq = read_u32(tcp_header, offset_of!(TcpHeader, seq));
        let ack = read_u32(tcp_header, offset_of!(TcpHeader, ack));
        let window_size = read_u16(tcp_header, offset_of!(TcpHeader, window_size));
        let flags = tcp_header[offset_of!(TcpHeader, flags)];
        let tcp_header_len = (tcp_header[offset_of!(TcpHeader, data_offset)] >> 4) as usize * 4;

        let payload_len = (total_len - (ip_header_len + tcp_header_len)) as u16;
        TcpPacketInfo {
            remote_ip,
            local_ip,
            remote_port,
            local_port,
            seq,
            ack,
            payload_len: payload_len as u16,
            window_size,
            flags,
        }
    }

    pub fn five_tuple(&self) -> FiveTuple {
        FiveTuple {
            eth_type: ETHTYPE_IPV4,
            ip_proto: IPPROTO_TCP,
            local_ip: self.local_ip,
            local_port: self.local_port,
            remote_ip: self.remote_ip,
            remote_port: self.remote_port,
        }
    }
}

fn read_u16(buf: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes(buf[offset..offset + size_of::<u16>()].try_into().unwrap())
}

fn read_u32(buf: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(buf[offset..offset + size_of::<u32>()].try_into().unwrap())
}

pub struct HeaderBuilder {
    header: [u8; IPV4_HEADER_LEN + TCP_HEADER_LEN],
}

impl HeaderBuilder {
    pub const fn new() -> Self {
        Self {
            header: [0u8; IPV4_HEADER_LEN + TCP_HEADER_LEN],
        }
    }

    pub fn build(
        mut self,
        remote: Endpoint,
        local_port: u16,
        segment: &Segment<'_>,
    ) -> [u8; IPV4_HEADER_LEN + TCP_HEADER_LEN] {
        self.write_ipv4_header(remote, segment);
        self.write_tcp_header(remote, local_port, segment);
        self.header
    }

    /// FIlls the IPv4 header.
    ///
    /// `checksum` and `src_ip` are filled by the kernel.
    fn write_ipv4_header(&mut self, remote: Endpoint, segment: &Segment<'_>) {
        let total_len = IPV4_HEADER_LEN + TCP_HEADER_LEN + segment.payload.len();
        self.write_u8(offset_of!(Ipv4Header, version_ihl), 0x45);
        self.write_u8(offset_of!(Ipv4Header, ttl), 64);
        self.write_u8(offset_of!(Ipv4Header, protocol), IPPROTO_TCP);
        self.write_u16(offset_of!(Ipv4Header, total_len), total_len as u16);
        self.write_u16(offset_of!(Ipv4Header, flags_and_frag), 0x4000);
        self.write_u32(offset_of!(Ipv4Header, src_ip), 0); // Kernel fills this
        self.write_u32(offset_of!(Ipv4Header, dst_ip), remote.ip);
    }

    /// FIlls the TCP header.
    ///
    /// `checksum` is filled by the kernel.
    fn write_tcp_header(&mut self, remote: Endpoint, local_port: u16, segment: &Segment<'_>) {
        let base = IPV4_HEADER_LEN;

        let window_size = segment.window_size;
        self.write_u8(base + offset_of!(TcpHeader, data_offset), 5 << 4);
        self.write_u8(base + offset_of!(TcpHeader, flags), segment.flags.as_u8());
        self.write_u16(base + offset_of!(TcpHeader, src_port), local_port);
        self.write_u16(base + offset_of!(TcpHeader, dst_port), remote.port);
        self.write_u32(base + offset_of!(TcpHeader, seq), segment.seq);
        self.write_u32(base + offset_of!(TcpHeader, ack), segment.ack);
        self.write_u16(base + offset_of!(TcpHeader, window_size), window_size);
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
