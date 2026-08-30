use core::ops::BitOr;
use core::ops::BitOrAssign;

const OUR_IP: u32 = 0x0a00_020f;
const IPV4_HEADER_LEN: usize = 20;
const TCP_HEADER_LEN: usize = 20;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Endpoint {
    pub ip: u32,
    pub port: u16,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct TcpFlags(u8);

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

pub(super) struct Segment<'a> {
    pub seq: u32,
    pub ack: u32,
    pub window_size: u16,
    pub flags: TcpFlags,
    pub payload: &'a [u8],
}

pub(super) fn build_header(
    remote: Endpoint,
    local_port: u16,
    segment: &Segment,
) -> [u8; IPV4_HEADER_LEN + TCP_HEADER_LEN] {
    let mut header = [0u8; IPV4_HEADER_LEN + TCP_HEADER_LEN];
    let tcp_len = TCP_HEADER_LEN + segment.payload.len();
    write_ipv4_header(&mut header[..IPV4_HEADER_LEN], remote.ip, tcp_len);

    let tcp_header = &mut header[IPV4_HEADER_LEN..];
    write_tcp_header(tcp_header, remote, local_port, segment);
    let checksum = tcp_checksum(remote.ip, tcp_header, segment.payload);
    tcp_header[16..18].copy_from_slice(&checksum.to_be_bytes());
    header
}

fn write_ipv4_header(header: &mut [u8], remote_ip: u32, tcp_len: usize) {
    let total_len = IPV4_HEADER_LEN + tcp_len;
    header[0] = 0x45;
    header[2..4].copy_from_slice(&(total_len as u16).to_be_bytes());
    header[6..8].copy_from_slice(&0x4000u16.to_be_bytes());
    header[8] = 64;
    header[9] = 6;
    header[12..16].copy_from_slice(&OUR_IP.to_be_bytes());
    header[16..20].copy_from_slice(&remote_ip.to_be_bytes());

    let checksum = checksum(&header[..IPV4_HEADER_LEN], 0);
    header[10..12].copy_from_slice(&checksum.to_be_bytes());
}

fn write_tcp_header(header: &mut [u8], remote: Endpoint, local_port: u16, segment: &Segment) {
    header[0..2].copy_from_slice(&local_port.to_be_bytes());
    header[2..4].copy_from_slice(&remote.port.to_be_bytes());
    header[4..8].copy_from_slice(&segment.seq.to_be_bytes());
    header[8..12].copy_from_slice(&segment.ack.to_be_bytes());
    header[12] = 5 << 4;
    header[13] = segment.flags.as_u8();
    header[14..16].copy_from_slice(&segment.window_size.to_be_bytes());
}

fn tcp_checksum(remote_ip: u32, header: &[u8], payload: &[u8]) -> u16 {
    let tcp_len = TCP_HEADER_LEN + payload.len();
    let mut sum = 0u32;
    sum = checksum_add(sum, &OUR_IP.to_be_bytes());
    sum = checksum_add(sum, &remote_ip.to_be_bytes());
    sum += 6;
    sum += tcp_len as u32;
    sum = checksum_add(sum, header);
    sum = checksum_add(sum, payload);
    checksum_finish(sum)
}

fn checksum(bytes: &[u8], initial: u32) -> u16 {
    let sum = checksum_add(initial, bytes);
    checksum_finish(sum)
}

fn checksum_add(mut sum: u32, bytes: &[u8]) -> u32 {
    let mut chunks = bytes.chunks_exact(2);
    for chunk in &mut chunks {
        let word = u16::from_be_bytes([chunk[0], chunk[1]]);
        sum += word as u32;
    }
    if let Some(byte) = chunks.remainder().first() {
        sum += (*byte as u32) << 8;
    }
    sum
}

fn checksum_finish(mut sum: u32) -> u16 {
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}
