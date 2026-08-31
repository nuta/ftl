use core::mem::offset_of;
use core::mem::size_of;

use super::helper::read_array;
use super::helper::read_u32;
use super::helper::write_array;
use super::helper::write_u16;
use super::helper::write_u32;
use super::ipv4::Ipv4Addr;

pub const DHCP_CLIENT_PORT: u16 = 68;
pub const DHCP_SERVER_PORT: u16 = 67;

pub const DHCP_OP_REQUEST: u8 = 1;
pub const DHCP_OP_REPLY: u8 = 2;
pub const DHCP_HW_ETHERNET: u8 = 1;
pub const DHCP_ETHERNET_ADDR_LEN: u8 = 6;
pub const DHCP_BROADCAST: u16 = 0x8000;

pub const DHCP_DISCOVER: u8 = 1;
pub const DHCP_OFFER: u8 = 2;
pub const DHCP_REQUEST: u8 = 3;
pub const DHCP_ACK: u8 = 5;
pub const DHCP_NAK: u8 = 6;

pub const OPTION_SUBNET_MASK: u8 = 1;
pub const OPTION_ROUTER: u8 = 3;
pub const OPTION_REQUESTED_IP: u8 = 50;
pub const OPTION_MESSAGE_TYPE: u8 = 53;
pub const OPTION_SERVER_ID: u8 = 54;
pub const OPTION_PARAM_REQUEST_LIST: u8 = 55;
pub const OPTION_PAD: u8 = 0;
pub const OPTION_END: u8 = 255;

const MAGIC_COOKIE: u32 = 0x6382_5363;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    TooShort,
    UnsupportedPacket,
    InvalidMagicCookie,
    MalformedOptions,
    OptionsFull,
}

#[repr(C, packed)]
struct DhcpHeader {
    op: u8,
    hw_type: u8,
    hwaddr_len: u8,
    hops: u8,
    tx_id: u32,
    seconds: u16,
    flags: u16,
    client_ip: u32,
    your_ip: u32,
    server_ip: u32,
    gateway_ip: u32,
    client_hwaddr: [u8; 16],
    server_name: [u8; 64],
    boot_file: [u8; 128],
    magic_cookie: u32,
}

pub const DHCP_HEADER_LEN: usize = size_of::<DhcpHeader>();

pub struct DhcpInspector<'a> {
    buf: &'a [u8],
}

impl<'a> DhcpInspector<'a> {
    pub fn new(buf: &'a [u8]) -> Result<Self, Error> {
        if buf.len() < DHCP_HEADER_LEN {
            return Err(Error::TooShort);
        }

        let packet = Self { buf };
        if packet.op() != DHCP_OP_REPLY
            || packet.hw_type() != DHCP_HW_ETHERNET
            || packet.hwaddr_len() != DHCP_ETHERNET_ADDR_LEN
        {
            return Err(Error::UnsupportedPacket);
        }
        if read_u32(buf, offset_of!(DhcpHeader, magic_cookie)) != MAGIC_COOKIE {
            return Err(Error::InvalidMagicCookie);
        }

        Ok(packet)
    }

    pub fn op(&self) -> u8 {
        self.buf[offset_of!(DhcpHeader, op)]
    }

    pub fn hw_type(&self) -> u8 {
        self.buf[offset_of!(DhcpHeader, hw_type)]
    }

    pub fn hwaddr_len(&self) -> u8 {
        self.buf[offset_of!(DhcpHeader, hwaddr_len)]
    }

    pub fn tx_id(&self) -> u32 {
        read_u32(self.buf, offset_of!(DhcpHeader, tx_id))
    }

    pub fn your_ip(&self) -> Ipv4Addr {
        Ipv4Addr::new(read_u32(self.buf, offset_of!(DhcpHeader, your_ip)))
    }

    pub fn client_hwaddr(&self) -> [u8; 6] {
        read_array(self.buf, offset_of!(DhcpHeader, client_hwaddr))
    }

    pub fn option(&self, wanted: u8) -> Result<Option<&'a [u8]>, Error> {
        let options = &self.buf[DHCP_HEADER_LEN..];
        let mut offset = 0;
        while offset < options.len() {
            let code = options[offset];
            offset += 1;
            match code {
                OPTION_PAD => continue,
                OPTION_END => return Ok(None),
                _ => {}
            }

            let Some(&len) = options.get(offset) else {
                return Err(Error::MalformedOptions);
            };
            offset += 1;
            let end = offset + len as usize;
            let Some(value) = options.get(offset..end) else {
                return Err(Error::MalformedOptions);
            };
            if code == wanted {
                return Ok(Some(value));
            }
            offset = end;
        }

        Ok(None)
    }

    pub fn message_type(&self) -> Result<Option<u8>, Error> {
        if let Some(value) = self.option(OPTION_MESSAGE_TYPE)? {
            if value.len() == 1 {
                return Ok(Some(value[0]));
            }
        }

        Ok(None)
    }

    pub fn ipv4_option(&self, code: u8) -> Result<Option<Ipv4Addr>, Error> {
        if let Some(value) = self.option(code)? {
            if let Some(ip_slice) = value.get(..4) {
                let ip_array = ip_slice.try_into().unwrap();
                let ip_u32 = u32::from_be_bytes(ip_array);
                return Ok(Some(Ipv4Addr::new(ip_u32)));
            }
        }

        Ok(None)
    }
}

pub struct DhcpRewriter<'a> {
    buf: &'a mut [u8],
    options_offset: usize,
}

impl<'a> DhcpRewriter<'a> {
    pub fn new(buf: &'a mut [u8]) -> Result<Self, Error> {
        if buf.len() < DHCP_HEADER_LEN {
            return Err(Error::TooShort);
        }

        buf.fill(0);
        write_u32(buf, offset_of!(DhcpHeader, magic_cookie), MAGIC_COOKIE);
        Ok(Self {
            buf,
            options_offset: DHCP_HEADER_LEN,
        })
    }

    pub fn set_op(&mut self, op: u8) {
        self.buf[offset_of!(DhcpHeader, op)] = op;
    }

    pub fn set_hw_type(&mut self, hardware_type: u8) {
        self.buf[offset_of!(DhcpHeader, hw_type)] = hardware_type;
    }

    pub fn set_hwaddr_len(&mut self, len: u8) {
        self.buf[offset_of!(DhcpHeader, hwaddr_len)] = len;
    }

    pub fn set_tx_id(&mut self, tx_id: u32) {
        write_u32(self.buf, offset_of!(DhcpHeader, tx_id), tx_id);
    }

    pub fn set_flags(&mut self, flags: u16) {
        write_u16(self.buf, offset_of!(DhcpHeader, flags), flags);
    }

    pub fn set_client_hwaddr(&mut self, mac: [u8; 6]) {
        write_array(self.buf, offset_of!(DhcpHeader, client_hwaddr), mac);
    }

    pub fn write_option(&mut self, code: u8, value: &[u8]) -> Result<(), Error> {
        if value.len() > u8::MAX as usize || self.options_offset + 2 + value.len() >= self.buf.len()
        {
            return Err(Error::OptionsFull);
        }

        self.buf[self.options_offset] = code;
        self.buf[self.options_offset + 1] = value.len() as u8;
        let start = self.options_offset + 2;
        self.buf[start..start + value.len()].copy_from_slice(value);
        self.options_offset = start + value.len();
        Ok(())
    }

    pub fn finish(&mut self) -> Result<(), Error> {
        if self.options_offset >= self.buf.len() {
            return Err(Error::OptionsFull);
        }

        self.buf[self.options_offset] = OPTION_END;
        Ok(())
    }
}
