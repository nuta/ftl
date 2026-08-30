use core::mem::offset_of;
use core::mem::size_of;

use ftl_types::net::IPPROTO_TCP;

use super::Error;
use super::checksum::Checksum;
use super::ipv4::Ipv4Addr;
use super::ipv4::Ipv4Inspector;
use super::read_u16;
use super::read_u32;
use super::write_u16;
use super::write_u32;

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

    pub fn validate_ipv4_checksum(&self, ipv4: &Ipv4Inspector<'_>) -> Result<(), Error> {
        if ipv4.ip_proto() != IPPROTO_TCP {
            return Err(Error::NotTcp);
        }

        let checksum = Checksum::new()
            .add(&ipv4.src_ip().as_u32().to_be_bytes())
            .add(&ipv4.dst_ip().as_u32().to_be_bytes())
            .add_u16(IPPROTO_TCP as u16)
            .add_u16(self.buf.len() as u16)
            .add(self.buf);
        if !checksum.is_valid() {
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
        let checksum = Checksum::new()
            .add(&src_ip.as_u32().to_be_bytes())
            .add(&dst_ip.as_u32().to_be_bytes())
            .add_u16(IPPROTO_TCP as u16)
            .add_u16(tcp_len)
            .add(&self.buf[..header_len])
            .add(payload)
            .value();
        self.set_checksum(checksum);
        Ok(())
    }
}
