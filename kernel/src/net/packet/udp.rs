use core::mem::offset_of;
use core::mem::size_of;

use super::checksum::Checksum;
use super::helper::read_u16;
use super::helper::write_u16;
use super::ipv4::Ipv4Addr;
use super::ipv4::Ipv4Inspector;

pub const IPPROTO_UDP: u8 = 17;
pub const UDP_HEADER_LEN: usize = size_of::<UdpHeader>();

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    NotUdp,
    TooShort,
    InvalidLength,
    InvalidChecksum,
}

#[repr(C, packed)]
struct UdpHeader {
    src_port: u16,
    dst_port: u16,
    len: u16,
    checksum: u16,
}

pub struct UdpInspector<'a> {
    buf: &'a [u8],
    len: usize,
}

impl<'a> UdpInspector<'a> {
    pub fn new(buf: &'a [u8]) -> Result<Self, Error> {
        if buf.len() < UDP_HEADER_LEN {
            return Err(Error::TooShort);
        }

        let len = read_u16(buf, offset_of!(UdpHeader, len)) as usize;
        if len < UDP_HEADER_LEN || len > buf.len() {
            return Err(Error::InvalidLength);
        }

        Ok(Self { buf, len })
    }

    pub fn validate(&self, ipv4: &Ipv4Inspector<'_>) -> Result<(), Error> {
        if ipv4.ip_proto() != IPPROTO_UDP {
            return Err(Error::NotUdp);
        }

        let expected = self.checksum();
        if expected == 0 {
            return Ok(());
        }

        let mut checksum = Checksum::new();
        checksum.add_ipv4(ipv4.src_ip());
        checksum.add_ipv4(ipv4.dst_ip());
        checksum.add_u16(IPPROTO_UDP as u16);
        checksum.add_u16(self.len as u16);
        if checksum.finish(&self.buf[..self.len]) != 0 {
            return Err(Error::InvalidChecksum);
        }

        Ok(())
    }

    pub fn src_port(&self) -> u16 {
        read_u16(self.buf, offset_of!(UdpHeader, src_port))
    }

    pub fn dst_port(&self) -> u16 {
        read_u16(self.buf, offset_of!(UdpHeader, dst_port))
    }

    pub fn checksum(&self) -> u16 {
        read_u16(self.buf, offset_of!(UdpHeader, checksum))
    }

    pub fn payload(&self) -> &'a [u8] {
        &self.buf[UDP_HEADER_LEN..self.len]
    }
}

pub struct UdpRewriter<'a> {
    buf: &'a mut [u8],
}

impl<'a> UdpRewriter<'a> {
    pub fn new(buf: &'a mut [u8]) -> Result<Self, Error> {
        if buf.len() < UDP_HEADER_LEN {
            return Err(Error::TooShort);
        }
        if buf.len() > u16::MAX as usize {
            return Err(Error::InvalidLength);
        }

        Ok(Self { buf })
    }

    pub fn set_src_port(&mut self, port: u16) {
        write_u16(self.buf, offset_of!(UdpHeader, src_port), port);
    }

    pub fn set_dst_port(&mut self, port: u16) {
        write_u16(self.buf, offset_of!(UdpHeader, dst_port), port);
    }

    pub fn set_len(&mut self) {
        write_u16(self.buf, offset_of!(UdpHeader, len), self.buf.len() as u16);
    }

    pub fn update_checksum(&mut self, src_ip: Ipv4Addr, dst_ip: Ipv4Addr) {
        write_u16(self.buf, offset_of!(UdpHeader, checksum), 0);
        let mut checksum = Checksum::new();
        checksum.add_ipv4(src_ip);
        checksum.add_ipv4(dst_ip);
        checksum.add_u16(IPPROTO_UDP as u16);
        checksum.add_u16(self.buf.len() as u16);
        let checksum = checksum.finish(self.buf);
        write_u16(
            self.buf,
            offset_of!(UdpHeader, checksum),
            if checksum == 0 { u16::MAX } else { checksum },
        );
    }
}
