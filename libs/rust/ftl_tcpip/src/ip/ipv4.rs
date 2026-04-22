use core::fmt;

use crate::endian::Ne;
use crate::packet::Packet;
use crate::packet::{self};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ipv4Addr(u32);

impl Ipv4Addr {
    pub const UNSPECIFIED: Self = Self(0);

    pub const fn new(addr: u32) -> Self {
        Self(addr)
    }
}

impl From<Ne<u32>> for Ipv4Addr {
    fn from(raw: Ne<u32>) -> Self {
        let value = raw.into();
        Self(value)
    }
}

impl fmt::Debug for Ipv4Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl fmt::Display for Ipv4Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}",
            self.0 >> 24,
            (self.0 >> 16) & 0xff,
            (self.0 >> 8) & 0xff,
            self.0 & 0xff
        )
    }
}

#[repr(C, packed)]
pub struct Ipv4Header {
    version: u8,
    ihl: u8,
    tos: u8,
    len: Ne<u16>,
    id: Ne<u16>,
    flags: Ne<u16>,
    frag_offset: Ne<u16>,
    ttl: u8,
    protocol: u8,
    checksum: Ne<u16>,
    src_addr: Ne<u32>,
    dst_addr: Ne<u32>,
}

#[derive(Debug)]
pub enum Error {
    PacketRead(packet::ReserveError),
    BadVersion(u8),
    BadHeaderLength(u8),
}

pub(crate) fn handle_rx(pkt: &mut Packet) -> Result<(), Error> {
    let header = pkt.read::<Ipv4Header>().map_err(Error::PacketRead)?;
    let src = Ipv4Addr::from(header.src_addr);
    let dst = Ipv4Addr::from(header.dst_addr);

    if header.version != 4 {
        return Err(Error::BadVersion(header.version));
    }

    if header.ihl != 5 {
        return Err(Error::BadHeaderLength(header.ihl));
    }

    info!("IPv4 packet: src: {}, dst: {}", src, dst);
    Ok(())
}
