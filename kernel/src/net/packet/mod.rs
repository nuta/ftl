mod arp;
mod checksum;
mod ethernet;
mod ipv4;
mod tcp;

pub use arp::ARP_HW_ETHERNET;
pub use arp::ARP_HWADDR_LEN;
pub use arp::ARP_IPADDR_LEN;
pub use arp::ARP_OP_REPLY;
pub use arp::ARP_OP_REQUEST;
pub use arp::ArpInspector;
pub use arp::ArpRewriter;
pub use ethernet::EthernetInspector;
pub use ethernet::EthernetRewriter;
pub use ipv4::Ipv4Addr;
pub use ipv4::Ipv4Inspector;
#[allow(unused_imports)]
pub use ipv4::Ipv4Rewriter;
pub use ipv4::NetMask;
pub use tcp::TcpInspector;
#[allow(unused_imports)]
pub use tcp::TcpRewriter;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    EthernetHeaderTooShort,
    ArpPacketTooShort,
    UnsupportedArpPacket,
    Ipv4HeaderTooShort,
    InvalidIpv4HeaderLength,
    NotIpv4,
    NotTcp,
    TcpHeaderTooShort,
    InvalidTcpHeaderLength,
    InvalidPacketLength,
    FragmentedPacket,
    InvalidIpv4Checksum,
    InvalidTcpChecksum,
}

pub(super) fn read_array<const N: usize>(buf: &[u8], offset: usize) -> [u8; N] {
    buf[offset..offset + N].try_into().unwrap()
}

pub(super) fn read_u16(buf: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes(read_array(buf, offset))
}

pub(super) fn read_u32(buf: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(read_array(buf, offset))
}

pub(super) fn write_array<const N: usize>(buf: &mut [u8], offset: usize, value: [u8; N]) {
    buf[offset..offset + N].copy_from_slice(&value);
}

pub(super) fn write_u16(buf: &mut [u8], offset: usize, value: u16) {
    write_array(buf, offset, value.to_be_bytes());
}

pub(super) fn write_u32(buf: &mut [u8], offset: usize, value: u32) {
    write_array(buf, offset, value.to_be_bytes());
}
