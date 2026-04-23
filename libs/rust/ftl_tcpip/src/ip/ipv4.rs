use core::fmt;

use crate::Io;
use crate::device::DeviceMap;
use crate::endian::Ne;
use crate::ip::IpAddr;
use crate::packet::Packet;
use crate::packet::{self};
use crate::route::RouteTable;
use crate::socket::SocketMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ipv4Addr(u32);

impl Ipv4Addr {
    pub const UNSPECIFIED: Self = Self(0);

    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self((a as u32) << 24 | (b as u32) << 16 | (c as u32) << 8 | d as u32)
    }
}

impl From<Ne<u32>> for Ipv4Addr {
    fn from(raw: Ne<u32>) -> Self {
        let value = raw.into();
        Self(value)
    }
}

impl From<Ipv4Addr> for Ne<u32> {
    fn from(addr: Ipv4Addr) -> Self {
        addr.0.into()
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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NetMask(u32);

impl NetMask {
    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self((a as u32) << 24 | (b as u32) << 16 | (c as u32) << 8 | d as u32)
    }

    pub fn contains(&self, our_addr: &Ipv4Addr, dest_addr: &Ipv4Addr) -> bool {
        (dest_addr.0 & self.0) == (our_addr.0 & self.0)
    }
}

impl fmt::Debug for NetMask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl fmt::Display for NetMask {
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
struct Ipv4Header {
    version_and_ihl: u8,
    tos: u8,
    len: Ne<u16>,
    id: Ne<u16>,
    flags_and_frag_offset: Ne<u16>,
    ttl: u8,
    protocol: u8,
    checksum: Ne<u16>,
    src_addr: Ne<u32>,
    dst_addr: Ne<u32>,
}

impl Ipv4Header {
    fn version(&self) -> u8 {
        self.version_and_ihl >> 4
    }

    fn ihl(&self) -> u8 {
        self.version_and_ihl & 0x0f
    }
}

#[derive(Debug)]
pub enum Error {
    PacketRead(packet::ReserveError),
    BadVersion(u8),
    BadHeaderLength(u8),
    UnsupportedProtocol(u8),
}

pub(crate) fn handle_rx<I: Io>(
    devices: &mut DeviceMap<I::Device>,
    routes: &mut RouteTable,
    sockets: &mut SocketMap,
    pkt: &mut Packet,
) -> Result<(), Error> {
    let header = pkt.read::<Ipv4Header>().map_err(Error::PacketRead)?;
    if header.version() != 4 {
        return Err(Error::BadVersion(header.version()));
    }

    if header.ihl() != 5 {
        return Err(Error::BadHeaderLength(header.ihl()));
    }

    let src = Ipv4Addr::from(header.src_addr);
    let remote = IpAddr::V4(src);
    let dst = Ipv4Addr::from(header.dst_addr);
    info!("IPv4 packet: src: {}, dst: {}", src, dst);

    match header.protocol {
        0x06 => {
            if let Err(err) =
                crate::transport::tcp::handle_rx::<I>(devices, routes, sockets, pkt, remote)
            {
                warn!("bad TCP packet: {:?}", err);
            }
        }
        protocol => {
            return Err(Error::UnsupportedProtocol(protocol));
        }
    }

    Ok(())
}
