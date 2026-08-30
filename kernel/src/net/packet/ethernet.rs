use core::mem::offset_of;
use core::mem::size_of;

use super::Error;
use super::read_array;
use super::read_u16;
use super::write_array;
use super::write_u16;

#[repr(C, packed)]
struct EthernetLayout {
    dst_mac: [u8; 6],
    src_mac: [u8; 6],
    eth_type: [u8; 2],
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
