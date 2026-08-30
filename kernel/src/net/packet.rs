use core::mem::offset_of;
use core::mem::size_of;

use ftl_types::net::ETHTYPE_IPV4;
use ftl_types::net::IPPROTO_TCP;

pub const ARP_HW_ETHERNET: u16 = 1;
pub const ARP_HWADDR_LEN: u8 = 6;
pub const ARP_IPADDR_LEN: u8 = 4;
pub const ARP_OP_REQUEST: u16 = 1;
pub const ARP_OP_REPLY: u16 = 2;

#[repr(C, packed)]
struct EthernetLayout {
    dst_mac: [u8; 6],
    src_mac: [u8; 6],
    eth_type: [u8; 2],
}

#[repr(C, packed)]
struct ArpLayout {
    hardware_type: [u8; 2],
    protocol_type: [u8; 2],
    hardware_addr_len: u8,
    protocol_addr_len: u8,
    operation: [u8; 2],
    src_mac: [u8; 6],
    src_ip: [u8; 4],
    dst_mac: [u8; 6],
    dst_ip: [u8; 4],
}

#[repr(C, packed)]
struct Ipv4Layout {
    version_ihl: u8,
    dscp_ecn: u8,
    total_len: [u8; 2],
    identification: [u8; 2],
    flags_fragment_offset: [u8; 2],
    ttl: u8,
    protocol: u8,
    checksum: [u8; 2],
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
}

#[repr(C, packed)]
struct TcpLayout {
    src_port: [u8; 2],
    dst_port: [u8; 2],
    seq: [u8; 4],
    ack: [u8; 4],
    data_offset: u8,
    flags: u8,
    window_size: [u8; 2],
    checksum: [u8; 2],
    urgent_pointer: [u8; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Ipv4Addr(u32);

impl Ipv4Addr {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NetMask(u32);

impl NetMask {
    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        (ip.0 & self.0) == self.0
    }
}

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

pub struct EthernetInspector<'a> {
    buf: &'a [u8],
}

impl<'a> EthernetInspector<'a> {
    pub const HEADER_LEN: usize = size_of::<EthernetLayout>();

    pub fn new(buf: &'a [u8]) -> Result<Self, Error> {
        if buf.len() < Self::HEADER_LEN {
            return Err(Error::EthernetHeaderTooShort);
        }

        Ok(Self { buf })
    }

    pub fn dst_mac(&self) -> [u8; 6] {
        read_array(self.buf, offset_of!(EthernetLayout, dst_mac))
    }

    pub fn src_mac(&self) -> [u8; 6] {
        read_array(self.buf, offset_of!(EthernetLayout, src_mac))
    }

    pub fn eth_type(&self) -> u16 {
        read_u16(self.buf, offset_of!(EthernetLayout, eth_type))
    }

    pub fn payload(&self) -> &'a [u8] {
        &self.buf[Self::HEADER_LEN..]
    }
}

pub struct EthernetRewriter<'a> {
    buf: &'a mut [u8],
}

impl<'a> EthernetRewriter<'a> {
    pub const HEADER_LEN: usize = size_of::<EthernetLayout>();

    pub fn new(buf: &'a mut [u8]) -> Result<Self, Error> {
        if buf.len() < Self::HEADER_LEN {
            return Err(Error::EthernetHeaderTooShort);
        }

        Ok(Self { buf })
    }

    pub fn set_dst_mac(&mut self, mac: [u8; 6]) {
        write_array(self.buf, offset_of!(EthernetLayout, dst_mac), mac);
    }

    pub fn set_src_mac(&mut self, mac: [u8; 6]) {
        write_array(self.buf, offset_of!(EthernetLayout, src_mac), mac);
    }

    pub fn set_eth_type(&mut self, eth_type: u16) {
        write_u16(self.buf, offset_of!(EthernetLayout, eth_type), eth_type);
    }
}

pub struct ArpInspector<'a> {
    buf: &'a [u8],
}

impl<'a> ArpInspector<'a> {
    pub const PACKET_LEN: usize = size_of::<ArpLayout>();

    pub fn new(buf: &'a [u8]) -> Result<Self, Error> {
        if buf.len() < Self::PACKET_LEN {
            return Err(Error::ArpPacketTooShort);
        }

        let inspector = Self { buf };
        if inspector.hardware_type() != ARP_HW_ETHERNET
            || inspector.protocol_type() != ETHTYPE_IPV4
            || inspector.hardware_addr_len() != ARP_HWADDR_LEN
            || inspector.protocol_addr_len() != ARP_IPADDR_LEN
        {
            return Err(Error::UnsupportedArpPacket);
        }

        Ok(inspector)
    }

    pub fn hardware_type(&self) -> u16 {
        read_u16(self.buf, offset_of!(ArpLayout, hardware_type))
    }

    pub fn protocol_type(&self) -> u16 {
        read_u16(self.buf, offset_of!(ArpLayout, protocol_type))
    }

    pub fn hardware_addr_len(&self) -> u8 {
        self.buf[offset_of!(ArpLayout, hardware_addr_len)]
    }

    pub fn protocol_addr_len(&self) -> u8 {
        self.buf[offset_of!(ArpLayout, protocol_addr_len)]
    }

    pub fn operation(&self) -> u16 {
        read_u16(self.buf, offset_of!(ArpLayout, operation))
    }

    pub fn src_mac(&self) -> [u8; 6] {
        read_array(self.buf, offset_of!(ArpLayout, src_mac))
    }

    pub fn src_ip(&self) -> Ipv4Addr {
        Ipv4Addr(read_u32(self.buf, offset_of!(ArpLayout, src_ip)))
    }

    pub fn dst_mac(&self) -> [u8; 6] {
        read_array(self.buf, offset_of!(ArpLayout, dst_mac))
    }

    pub fn dst_ip(&self) -> Ipv4Addr {
        Ipv4Addr(read_u32(self.buf, offset_of!(ArpLayout, dst_ip)))
    }
}

pub struct ArpRewriter<'a> {
    buf: &'a mut [u8],
}

impl<'a> ArpRewriter<'a> {
    pub const PACKET_LEN: usize = size_of::<ArpLayout>();

    pub fn new(buf: &'a mut [u8]) -> Result<Self, Error> {
        if buf.len() < Self::PACKET_LEN {
            return Err(Error::ArpPacketTooShort);
        }

        Ok(Self { buf })
    }

    pub fn set_hardware_type(&mut self, hardware_type: u16) {
        write_u16(
            self.buf,
            offset_of!(ArpLayout, hardware_type),
            hardware_type,
        );
    }

    pub fn set_protocol_type(&mut self, protocol_type: u16) {
        write_u16(
            self.buf,
            offset_of!(ArpLayout, protocol_type),
            protocol_type,
        );
    }

    pub fn set_hardware_addr_len(&mut self, len: u8) {
        self.buf[offset_of!(ArpLayout, hardware_addr_len)] = len;
    }

    pub fn set_protocol_addr_len(&mut self, len: u8) {
        self.buf[offset_of!(ArpLayout, protocol_addr_len)] = len;
    }

    pub fn set_operation(&mut self, operation: u16) {
        write_u16(self.buf, offset_of!(ArpLayout, operation), operation);
    }

    pub fn set_src_mac(&mut self, mac: [u8; 6]) {
        write_array(self.buf, offset_of!(ArpLayout, src_mac), mac);
    }

    pub fn set_src_ip(&mut self, ip: Ipv4Addr) {
        write_u32(self.buf, offset_of!(ArpLayout, src_ip), ip.as_u32());
    }

    pub fn set_dst_mac(&mut self, mac: [u8; 6]) {
        write_array(self.buf, offset_of!(ArpLayout, dst_mac), mac);
    }

    pub fn set_dst_ip(&mut self, ip: Ipv4Addr) {
        write_u32(self.buf, offset_of!(ArpLayout, dst_ip), ip.as_u32());
    }
}

pub struct Ipv4Inspector<'a> {
    buf: &'a [u8],
    header_len: usize,
    total_len: usize,
}

impl<'a> Ipv4Inspector<'a> {
    pub const MIN_HEADER_LEN: usize = size_of::<Ipv4Layout>();

    pub fn new(buf: &'a [u8]) -> Result<Self, Error> {
        if buf.len() < Self::MIN_HEADER_LEN {
            return Err(Error::Ipv4HeaderTooShort);
        }

        let version_ihl = buf[offset_of!(Ipv4Layout, version_ihl)];
        if version_ihl >> 4 != 4 {
            return Err(Error::NotIpv4);
        }

        let header_len = (version_ihl & 0x0f) as usize * 4;
        if header_len < Self::MIN_HEADER_LEN || buf.len() < header_len {
            return Err(Error::InvalidIpv4HeaderLength);
        }

        let total_len = read_u16(buf, offset_of!(Ipv4Layout, total_len)) as usize;
        if total_len < header_len {
            return Err(Error::InvalidPacketLength);
        }

        Ok(Self {
            buf,
            header_len,
            total_len,
        })
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.total_len > self.buf.len() {
            return Err(Error::InvalidPacketLength);
        }

        if self.flags_fragment_offset() & 0x3fff != 0 {
            return Err(Error::FragmentedPacket);
        }

        if !checksum_valid(&self.buf[..self.header_len], 0) {
            return Err(Error::InvalidIpv4Checksum);
        }

        Ok(())
    }

    pub fn packet_len(&self) -> usize {
        self.total_len
    }

    pub fn ip_proto(&self) -> u8 {
        self.buf[offset_of!(Ipv4Layout, protocol)]
    }

    pub fn dst_ip(&self) -> Ipv4Addr {
        Ipv4Addr(read_u32(self.buf, offset_of!(Ipv4Layout, dst_ip)))
    }

    pub fn src_ip(&self) -> Ipv4Addr {
        Ipv4Addr(read_u32(self.buf, offset_of!(Ipv4Layout, src_ip)))
    }

    pub fn flags_fragment_offset(&self) -> u16 {
        read_u16(self.buf, offset_of!(Ipv4Layout, flags_fragment_offset))
    }

    pub fn header_len(&self) -> usize {
        self.header_len
    }

    pub fn payload_len(&self) -> usize {
        self.total_len - self.header_len
    }

    pub fn payload(&self) -> Result<&'a [u8], Error> {
        if self.total_len > self.buf.len() {
            return Err(Error::InvalidPacketLength);
        }

        Ok(&self.buf[self.header_len..self.total_len])
    }
}

pub struct Ipv4Rewriter<'a> {
    buf: &'a mut [u8],
}

impl<'a> Ipv4Rewriter<'a> {
    pub const MIN_HEADER_LEN: usize = size_of::<Ipv4Layout>();

    pub fn new(buf: &'a mut [u8]) -> Result<Self, Error> {
        if buf.len() < Self::MIN_HEADER_LEN {
            return Err(Error::Ipv4HeaderTooShort);
        }

        Ok(Self { buf })
    }

    pub fn set_version(&mut self, version: u8) {
        let offset = offset_of!(Ipv4Layout, version_ihl);
        self.buf[offset] = (version << 4) | (self.buf[offset] & 0x0f);
    }

    pub fn set_header_len(&mut self, header_len: usize) -> Result<(), Error> {
        if header_len < Self::MIN_HEADER_LEN
            || header_len > 60
            || header_len % 4 != 0
            || header_len > self.buf.len()
        {
            return Err(Error::InvalidIpv4HeaderLength);
        }

        let offset = offset_of!(Ipv4Layout, version_ihl);
        self.buf[offset] = (self.buf[offset] & 0xf0) | ((header_len / 4) as u8);
        Ok(())
    }

    pub fn set_dscp_ecn(&mut self, dscp_ecn: u8) {
        self.buf[offset_of!(Ipv4Layout, dscp_ecn)] = dscp_ecn;
    }

    pub fn set_total_len(&mut self, total_len: usize) -> Result<(), Error> {
        let total_len = u16::try_from(total_len).map_err(|_| Error::InvalidPacketLength)?;
        write_u16(self.buf, offset_of!(Ipv4Layout, total_len), total_len);
        Ok(())
    }

    pub fn set_identification(&mut self, identification: u16) {
        write_u16(
            self.buf,
            offset_of!(Ipv4Layout, identification),
            identification,
        );
    }

    pub fn set_flags_fragment_offset(&mut self, value: u16) {
        write_u16(
            self.buf,
            offset_of!(Ipv4Layout, flags_fragment_offset),
            value,
        );
    }

    pub fn set_ttl(&mut self, ttl: u8) {
        self.buf[offset_of!(Ipv4Layout, ttl)] = ttl;
    }

    pub fn set_ip_proto(&mut self, protocol: u8) {
        self.buf[offset_of!(Ipv4Layout, protocol)] = protocol;
    }

    pub fn set_checksum(&mut self, checksum: u16) {
        write_u16(self.buf, offset_of!(Ipv4Layout, checksum), checksum);
    }

    pub fn set_src_ip(&mut self, ip: Ipv4Addr) {
        write_u32(self.buf, offset_of!(Ipv4Layout, src_ip), ip.as_u32());
    }

    pub fn set_dst_ip(&mut self, ip: Ipv4Addr) {
        write_u32(self.buf, offset_of!(Ipv4Layout, dst_ip), ip.as_u32());
    }

    pub fn recompute_checksum(&mut self) -> Result<(), Error> {
        let header_len = (self.buf[offset_of!(Ipv4Layout, version_ihl)] & 0x0f) as usize * 4;
        if header_len < Self::MIN_HEADER_LEN || header_len > self.buf.len() {
            return Err(Error::InvalidIpv4HeaderLength);
        }

        self.set_checksum(0);
        let checksum = checksum_value(&self.buf[..header_len], 0);
        self.set_checksum(checksum);
        Ok(())
    }
}

pub struct TcpInspector<'a> {
    buf: &'a [u8],
    header_len: usize,
}

impl<'a> TcpInspector<'a> {
    pub const MIN_HEADER_LEN: usize = size_of::<TcpLayout>();

    pub fn new(buf: &'a [u8]) -> Result<Self, Error> {
        if buf.len() < Self::MIN_HEADER_LEN {
            return Err(Error::TcpHeaderTooShort);
        }

        let header_len = (buf[offset_of!(TcpLayout, data_offset)] >> 4) as usize * 4;
        if header_len < Self::MIN_HEADER_LEN || buf.len() < header_len {
            return Err(Error::InvalidTcpHeaderLength);
        }

        Ok(Self { buf, header_len })
    }

    pub fn validate(&self, ipv4: &Ipv4Inspector<'_>) -> Result<(), Error> {
        if ipv4.ip_proto() != IPPROTO_TCP {
            return Err(Error::NotTcp);
        }

        let mut sum = 0;
        sum = checksum_add(sum, &ipv4.src_ip().as_u32().to_be_bytes());
        sum = checksum_add(sum, &ipv4.dst_ip().as_u32().to_be_bytes());
        sum += IPPROTO_TCP as u32;
        sum += self.buf.len() as u32;

        if !checksum_valid(self.buf, sum) {
            return Err(Error::InvalidTcpChecksum);
        }

        Ok(())
    }

    pub fn dst_port(&self) -> u16 {
        read_u16(self.buf, offset_of!(TcpLayout, dst_port))
    }

    pub fn src_port(&self) -> u16 {
        read_u16(self.buf, offset_of!(TcpLayout, src_port))
    }

    pub fn seq(&self) -> u32 {
        read_u32(self.buf, offset_of!(TcpLayout, seq))
    }

    pub fn ack(&self) -> u32 {
        read_u32(self.buf, offset_of!(TcpLayout, ack))
    }

    pub fn flags(&self) -> u8 {
        self.buf[offset_of!(TcpLayout, flags)]
    }

    pub fn window_size(&self) -> u16 {
        read_u16(self.buf, offset_of!(TcpLayout, window_size))
    }

    pub fn header_len(&self) -> usize {
        self.header_len
    }

    pub fn payload_len(&self) -> usize {
        self.buf.len() - self.header_len
    }
}

pub struct TcpRewriter<'a> {
    buf: &'a mut [u8],
}

impl<'a> TcpRewriter<'a> {
    pub const MIN_HEADER_LEN: usize = size_of::<TcpLayout>();

    pub fn new(buf: &'a mut [u8]) -> Result<Self, Error> {
        if buf.len() < Self::MIN_HEADER_LEN {
            return Err(Error::TcpHeaderTooShort);
        }

        Ok(Self { buf })
    }

    pub fn set_src_port(&mut self, port: u16) {
        write_u16(self.buf, offset_of!(TcpLayout, src_port), port);
    }

    pub fn set_dst_port(&mut self, port: u16) {
        write_u16(self.buf, offset_of!(TcpLayout, dst_port), port);
    }

    pub fn set_seq(&mut self, seq: u32) {
        write_u32(self.buf, offset_of!(TcpLayout, seq), seq);
    }

    pub fn set_ack(&mut self, ack: u32) {
        write_u32(self.buf, offset_of!(TcpLayout, ack), ack);
    }

    pub fn set_header_len(&mut self, header_len: usize) -> Result<(), Error> {
        if header_len < Self::MIN_HEADER_LEN
            || header_len > 60
            || header_len % 4 != 0
            || header_len > self.buf.len()
        {
            return Err(Error::InvalidTcpHeaderLength);
        }

        let offset = offset_of!(TcpLayout, data_offset);
        self.buf[offset] = ((header_len / 4) as u8) << 4 | (self.buf[offset] & 0x0f);
        Ok(())
    }

    pub fn set_flags(&mut self, flags: u8) {
        self.buf[offset_of!(TcpLayout, flags)] = flags;
    }

    pub fn set_window_size(&mut self, window_size: u16) {
        write_u16(self.buf, offset_of!(TcpLayout, window_size), window_size);
    }

    pub fn set_checksum(&mut self, checksum: u16) {
        write_u16(self.buf, offset_of!(TcpLayout, checksum), checksum);
    }

    pub fn set_urgent_pointer(&mut self, urgent_pointer: u16) {
        write_u16(
            self.buf,
            offset_of!(TcpLayout, urgent_pointer),
            urgent_pointer,
        );
    }

    pub fn recompute_checksum(
        &mut self,
        src_ip: Ipv4Addr,
        dst_ip: Ipv4Addr,
        payload: &[u8],
    ) -> Result<(), Error> {
        let header_len = (self.buf[offset_of!(TcpLayout, data_offset)] >> 4) as usize * 4;
        if header_len < Self::MIN_HEADER_LEN || header_len > self.buf.len() {
            return Err(Error::InvalidTcpHeaderLength);
        }

        let tcp_len = header_len
            .checked_add(payload.len())
            .and_then(|len| u16::try_from(len).ok())
            .ok_or(Error::InvalidPacketLength)?;

        self.set_checksum(0);
        let mut sum = 0;
        sum = checksum_add(sum, &src_ip.as_u32().to_be_bytes());
        sum = checksum_add(sum, &dst_ip.as_u32().to_be_bytes());
        sum += IPPROTO_TCP as u32;
        sum += tcp_len as u32;
        sum = checksum_add(sum, &self.buf[..header_len]);
        let checksum = checksum_value(payload, sum);
        self.set_checksum(checksum);
        Ok(())
    }
}

fn read_array<const N: usize>(buf: &[u8], offset: usize) -> [u8; N] {
    buf[offset..offset + N].try_into().unwrap()
}

fn read_u16(buf: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes(read_array(buf, offset))
}

fn read_u32(buf: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(read_array(buf, offset))
}

fn write_array<const N: usize>(buf: &mut [u8], offset: usize, value: [u8; N]) {
    buf[offset..offset + N].copy_from_slice(&value);
}

fn write_u16(buf: &mut [u8], offset: usize, value: u16) {
    write_array(buf, offset, value.to_be_bytes());
}

fn write_u32(buf: &mut [u8], offset: usize, value: u32) {
    write_array(buf, offset, value.to_be_bytes());
}

fn checksum_valid(bytes: &[u8], initial: u32) -> bool {
    checksum_fold(checksum_add(initial, bytes)) == 0xffff
}

fn checksum_value(bytes: &[u8], initial: u32) -> u16 {
    !checksum_fold(checksum_add(initial, bytes))
}

fn checksum_fold(mut sum: u32) -> u16 {
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    sum as u16
}

fn checksum_add(mut sum: u32, bytes: &[u8]) -> u32 {
    let mut chunks = bytes.chunks_exact(2);
    for chunk in &mut chunks {
        sum += u16::from_be_bytes([chunk[0], chunk[1]]) as u32;
    }

    if let Some(byte) = chunks.remainder().first() {
        sum += (*byte as u32) << 8;
    }
    sum
}
