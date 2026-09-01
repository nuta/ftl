use core::mem::offset_of;
use core::mem::size_of;

use ftl_types::net::IPPROTO_TCP;

use super::checksum::Checksum;
use super::helper::read_u16;
use super::helper::read_u32;
use super::helper::write_u16;
use super::ipv4::Ipv4Addr;
use super::ipv4::Ipv4Inspector;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    NotTcp,
    TcpHeaderTooShort,
    InvalidTcpHeaderLength,
    InvalidTcpChecksum,
}

const MIN_HEADER_LEN: usize = size_of::<TcpHeader>();

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

pub struct TcpInspector<'a> {
    buf: &'a [u8],
    header_len: usize,
}

impl<'a> TcpInspector<'a> {
    pub fn new(buf: &'a [u8]) -> Result<Self, Error> {
        if buf.len() < MIN_HEADER_LEN {
            return Err(Error::TcpHeaderTooShort);
        }

        let data_offset = buf[offset_of!(TcpHeader, data_offset)];
        let header_len = (data_offset >> 4) as usize * 4;
        if header_len < MIN_HEADER_LEN || buf.len() < header_len {
            return Err(Error::InvalidTcpHeaderLength);
        }

        Ok(Self { buf, header_len })
    }

    pub fn validate(&self, ipv4: &Ipv4Inspector<'_>) -> Result<(), Error> {
        if ipv4.ip_proto() != IPPROTO_TCP {
            return Err(Error::NotTcp);
        }

        let mut checksum = Checksum::new();
        checksum.add_ipv4(ipv4.src_ip());
        checksum.add_ipv4(ipv4.dst_ip());
        checksum.add_u16(IPPROTO_TCP as u16);
        checksum.add_u16(self.buf.len() as u16);
        checksum.add_bytes(self.buf);
        if checksum.finish() != 0 {
            return Err(Error::InvalidTcpChecksum);
        }

        Ok(())
    }

    pub fn dst_port(&self) -> u16 {
        read_u16(self.buf, offset_of!(TcpHeader, dst_port))
    }

    pub fn src_port(&self) -> u16 {
        read_u16(self.buf, offset_of!(TcpHeader, src_port))
    }

    pub fn seq(&self) -> u32 {
        read_u32(self.buf, offset_of!(TcpHeader, seq))
    }

    pub fn ack(&self) -> u32 {
        read_u32(self.buf, offset_of!(TcpHeader, ack))
    }

    pub fn flags(&self) -> u8 {
        self.buf[offset_of!(TcpHeader, flags)]
    }

    pub fn window_size(&self) -> u16 {
        read_u16(self.buf, offset_of!(TcpHeader, window_size))
    }

    pub fn checksum(&self) -> u16 {
        read_u16(self.buf, offset_of!(TcpHeader, checksum))
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
    pub fn new(buf: &'a mut [u8]) -> Result<Self, Error> {
        if buf.len() < MIN_HEADER_LEN {
            return Err(Error::TcpHeaderTooShort);
        }

        Ok(Self { buf })
    }

    pub fn update_checksum(&mut self, src_ip: Ipv4Addr, dst_ip: Ipv4Addr, payload: Option<&[u8]>) {
        write_u16(self.buf, offset_of!(TcpHeader, checksum), 0);

        let payload_len = if let Some(payload) = payload {
            payload.len()
        } else {
            0
        };

        let mut checksum = Checksum::new();
        checksum.add_ipv4(src_ip);
        checksum.add_ipv4(dst_ip);
        checksum.add_u16(IPPROTO_TCP as u16);
        checksum.add_u16((self.buf.len() + payload_len) as u16);
        checksum.add_bytes(self.buf);

        if let Some(payload) = payload {
            checksum.add_bytes(payload);
        }

        let checksum = checksum.finish();
        write_u16(self.buf, offset_of!(TcpHeader, checksum), checksum);
    }
}
