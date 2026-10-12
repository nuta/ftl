use core::mem::size_of;

#[repr(C, packed)]
pub struct Ipv4Header {
    pub version_ihl: u8,
    pub dscp_ecn: u8,
    pub total_len: u16,
    pub id: u16,
    pub flags_and_frag: u16,
    pub ttl: u8,
    pub protocol: u8,
    pub checksum: u16,
    pub src_ip: u32,
    pub dst_ip: u32,
}

pub const IPV4_HEADER_LEN: usize = size_of::<Ipv4Header>();

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Endpoint {
    pub ip: u32,
    pub port: u16,
}
