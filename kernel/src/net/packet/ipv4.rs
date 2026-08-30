use core::mem::offset_of;
use core::mem::size_of;

use super::Error;
use super::checksum::Checksum;
use super::read_u16;
use super::read_u32;
use super::write_u16;
use super::write_u32;

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

    pub fn validate_packet(&self) -> Result<(), Error> {
        if self.total_len > self.buf.len() {
            return Err(Error::InvalidPacketLength);
        }

        if self.flags_fragment_offset() & 0x3fff != 0 {
            return Err(Error::FragmentedPacket);
        }

        if !Checksum::new().add(&self.buf[..self.header_len]).is_valid() {
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
        let checksum = Checksum::new().add(&self.buf[..header_len]).value();
        self.set_checksum(checksum);
        Ok(())
    }
}
