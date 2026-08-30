use core::mem::offset_of;
use core::mem::size_of;

use ftl_types::net::ETHTYPE_IPV4;

use super::helper::read_array;
use super::helper::read_u16;
use super::helper::read_u32;
use super::helper::write_array;
use super::helper::write_u16;
use super::helper::write_u32;
use super::ipv4::Ipv4Addr;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    ArpPacketTooShort,
    UnsupportedArpPacket,
}

pub const ARP_HW_ETHERNET: u16 = 1;
pub const ARP_HWADDR_LEN: u8 = 6;
pub const ARP_IPADDR_LEN: u8 = 4;
pub const ARP_OP_REQUEST: u16 = 1;
pub const ARP_OP_REPLY: u16 = 2;

#[repr(C, packed)]
struct ArpLayout {
    hardware_type: u16,
    protocol_type: u16,
    hardware_addr_len: u8,
    protocol_addr_len: u8,
    operation: u16,
    src_mac: [u8; 6],
    src_ip: u32,
    dst_mac: [u8; 6],
    dst_ip: u32,
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
        if inspector.hw_type() != ARP_HW_ETHERNET
            || inspector.protocol_type() != ETHTYPE_IPV4
            || inspector.hwaddr_len() != ARP_HWADDR_LEN
            || inspector.ipaddr_len() != ARP_IPADDR_LEN
        {
            return Err(Error::UnsupportedArpPacket);
        }

        Ok(inspector)
    }

    pub fn hw_type(&self) -> u16 {
        read_u16(self.buf, offset_of!(ArpLayout, hardware_type))
    }

    pub fn protocol_type(&self) -> u16 {
        read_u16(self.buf, offset_of!(ArpLayout, protocol_type))
    }

    pub fn hwaddr_len(&self) -> u8 {
        self.buf[offset_of!(ArpLayout, hardware_addr_len)]
    }

    pub fn ipaddr_len(&self) -> u8 {
        self.buf[offset_of!(ArpLayout, protocol_addr_len)]
    }

    pub fn op(&self) -> u16 {
        read_u16(self.buf, offset_of!(ArpLayout, operation))
    }

    pub fn src_mac(&self) -> [u8; 6] {
        read_array(self.buf, offset_of!(ArpLayout, src_mac))
    }

    pub fn src_ip(&self) -> Ipv4Addr {
        Ipv4Addr::new(read_u32(self.buf, offset_of!(ArpLayout, src_ip)))
    }

    pub fn dst_mac(&self) -> [u8; 6] {
        read_array(self.buf, offset_of!(ArpLayout, dst_mac))
    }

    pub fn dst_ip(&self) -> Ipv4Addr {
        Ipv4Addr::new(read_u32(self.buf, offset_of!(ArpLayout, dst_ip)))
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
