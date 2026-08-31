use core::fmt;
use core::mem::offset_of;
use core::mem::size_of;

use super::checksum::Checksum;
use super::helper::read_u16;
use super::helper::read_u32;
use super::helper::write_u16;
use super::helper::write_u32;

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

impl fmt::Display for Ipv4Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [a, b, c, d] = self.0.to_be_bytes();
        write!(f, "{a}.{b}.{c}.{d}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NetMask(u32);

impl NetMask {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        (ip.0 & self.0) == self.0
    }
}

impl fmt::Display for NetMask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ipv4Addr(self.0).fmt(f)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    TooShort,
    InvalidHeaderLength,
    NotIpv4,
    InvalidPacketLength,
    Fragmented,
    InvalidChecksum,
}

const MIN_HEADER_LEN: usize = size_of::<Ipv4Header>();

#[repr(C, packed)]
struct Ipv4Header {
    version_ihl: u8,
    dscp_ecn: u8,
    total_len: u16,
    identification: u16,
    flags_fragment_offset: u16,
    ttl: u8,
    protocol: u8,
    checksum: u16,
    src_ip: u32,
    dst_ip: u32,
}

pub struct Ipv4Inspector<'a> {
    buf: &'a [u8],
    header_len: usize,
    total_len: usize,
}

impl<'a> Ipv4Inspector<'a> {
    pub fn new(buf: &'a [u8]) -> Result<Self, Error> {
        if buf.len() < MIN_HEADER_LEN {
            return Err(Error::TooShort);
        }

        let version_ihl = buf[offset_of!(Ipv4Header, version_ihl)];
        if version_ihl >> 4 != 4 {
            return Err(Error::NotIpv4);
        }

        let header_len = (version_ihl & 0x0f) as usize * 4;
        if header_len < MIN_HEADER_LEN || buf.len() < header_len {
            return Err(Error::InvalidHeaderLength);
        }

        let total_len = read_u16(buf, offset_of!(Ipv4Header, total_len)) as usize;
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

        if self.fragment_offset() & 0x3fff != 0 {
            return Err(Error::Fragmented);
        }

        // Calculate the checksum, including the checksum field.
        let checksum = Checksum::new();
        let ipv4_header_bytes = &self.buf[..self.header_len];
        if checksum.finish(ipv4_header_bytes) != 0 {
            return Err(Error::InvalidChecksum);
        }

        Ok(())
    }

    pub fn total_len(&self) -> usize {
        self.total_len
    }

    pub fn ip_proto(&self) -> u8 {
        self.buf[offset_of!(Ipv4Header, protocol)]
    }

    pub fn checksum(&self) -> u16 {
        read_u16(self.buf, offset_of!(Ipv4Header, checksum))
    }

    pub fn dst_ip(&self) -> Ipv4Addr {
        Ipv4Addr(read_u32(self.buf, offset_of!(Ipv4Header, dst_ip)))
    }

    pub fn src_ip(&self) -> Ipv4Addr {
        Ipv4Addr(read_u32(self.buf, offset_of!(Ipv4Header, src_ip)))
    }

    pub fn fragment_offset(&self) -> u16 {
        read_u16(self.buf, offset_of!(Ipv4Header, flags_fragment_offset))
    }

    pub fn header_len(&self) -> usize {
        self.header_len
    }

    pub fn payload_len(&self) -> usize {
        self.total_len - self.header_len
    }

    pub fn payload(&self) -> &'a [u8] {
        // TODO: How should we enforce the header_len/total_len validation?
        &self.buf[self.header_len..self.total_len]
    }
}

pub struct Ipv4Rewriter<'a> {
    buf: &'a mut [u8],
}

impl<'a> Ipv4Rewriter<'a> {
    pub const HEADER_LEN: usize = MIN_HEADER_LEN;

    pub fn new(buf: &'a mut [u8]) -> Result<Self, Error> {
        if buf.len() < Self::HEADER_LEN {
            return Err(Error::TooShort);
        }

        Ok(Self { buf })
    }

    pub fn set_version_and_header_len(&mut self) {
        self.buf[offset_of!(Ipv4Header, version_ihl)] = 0x45;
    }

    pub fn set_total_len(&mut self, total_len: u16) {
        write_u16(self.buf, offset_of!(Ipv4Header, total_len), total_len);
    }

    pub fn set_identification(&mut self, identification: u16) {
        write_u16(
            self.buf,
            offset_of!(Ipv4Header, identification),
            identification,
        );
    }

    pub fn set_flags_and_fragment_offset(&mut self, value: u16) {
        write_u16(
            self.buf,
            offset_of!(Ipv4Header, flags_fragment_offset),
            value,
        );
    }

    pub fn set_ttl(&mut self, ttl: u8) {
        self.buf[offset_of!(Ipv4Header, ttl)] = ttl;
    }

    pub fn set_ip_proto(&mut self, protocol: u8) {
        self.buf[offset_of!(Ipv4Header, protocol)] = protocol;
    }

    pub fn set_src_ip(&mut self, ip: Ipv4Addr) {
        write_u32(self.buf, offset_of!(Ipv4Header, src_ip), ip.as_u32());
    }

    pub fn set_dst_ip(&mut self, ip: Ipv4Addr) {
        write_u32(self.buf, offset_of!(Ipv4Header, dst_ip), ip.as_u32());
    }

    pub fn update_checksum(&mut self) {
        write_u16(self.buf, offset_of!(Ipv4Header, checksum), 0);
        let checksum = Checksum::new().finish(&self.buf[..Self::HEADER_LEN]);
        write_u16(self.buf, offset_of!(Ipv4Header, checksum), checksum);
    }
}
