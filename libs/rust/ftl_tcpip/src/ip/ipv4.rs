use core::fmt;

use crate::checksum::Checksum;
use crate::device::DeviceMap;
use crate::endian::Ne;
use crate::ethernet;
use crate::ethernet::EtherType;
use crate::io::Io;
use crate::ip::IpAddr;
use crate::packet;
use crate::packet::Packet;
use crate::packet::WriteableToPacket;
use crate::route::Route;
use crate::route::RouteTable;
use crate::socket::SocketMap;
use crate::transport;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ipv4Addr(u32);

impl Ipv4Addr {
    pub const UNSPECIFIED: Self = Self(0);

    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self((a as u32) << 24 | (b as u32) << 16 | (c as u32) << 8 | d as u32)
    }

    pub(crate) fn as_u32(self) -> u32 {
        self.0
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
pub(crate) struct Ipv4Header {
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

impl WriteableToPacket for Ipv4Header {}

impl Ipv4Header {
    fn version(&self) -> u8 {
        self.version_and_ihl >> 4
    }

    fn ihl(&self) -> u8 {
        self.version_and_ihl & 0x0f
    }

    fn compute_checksum(&self) -> u16 {
        let mut checksum = Checksum::new();
        checksum.supply_u16(((self.version_and_ihl as u16) << 8) | self.tos as u16);
        checksum.supply_u16(self.len.into());
        checksum.supply_u16(self.id.into());
        checksum.supply_u16(self.flags_and_frag_offset.into());
        checksum.supply_u16(((self.ttl as u16) << 8) | self.protocol as u16);
        checksum.supply_u16(0);
        checksum.supply_u32(self.src_addr.into());
        checksum.supply_u32(self.dst_addr.into());
        checksum.finish()
    }
}

#[derive(Debug)]
pub enum TxError {
    NoRoute,
    NoDevice,
    PacketWrite(packet::ReserveError),
    EthernetTx(ethernet::TxError),
}

pub(crate) fn transmit<I: Io>(
    device: &mut I::Device,
    route: &Route,
    pkt: &mut Packet,
    src_ip: Ipv4Addr,
    dest_ip: Ipv4Addr,
    protocol: transport::Protocol,
) -> Result<(), TxError> {
    let mut header = Ipv4Header {
        version_and_ihl: 4 << 4 | 5,
        tos: 0,
        len: ((size_of::<Ipv4Header>() + pkt.len()) as u16).into(),
        id: 0.into(),
        flags_and_frag_offset: 0.into(),
        ttl: 64,
        protocol: protocol as u8,
        checksum: 0.into(),
        src_addr: src_ip.into(),
        dst_addr: dest_ip.into(),
    };
    header.checksum = header.compute_checksum().into();

    pkt.write_front(header).map_err(TxError::PacketWrite)?;
    ethernet::transmit(device, route, EtherType::Ipv4, pkt, IpAddr::V4(dest_ip))
        .map_err(TxError::EthernetTx)?;
    Ok(())
}

#[derive(Debug)]
pub enum RxError {
    PacketRead(packet::ReserveError),
    BadVersion(u8),
    BadHeaderLength(u8),
    UnsupportedProtocol(u8),
    Tcp(crate::tcp::RxError),
}

pub(crate) fn handle_rx<I: Io>(
    devices: &mut DeviceMap<I::Device>,
    routes: &mut RouteTable,
    sockets: &mut SocketMap,
    pkt: &mut Packet,
) -> Result<(), RxError> {
    let header = pkt.read::<Ipv4Header>().map_err(RxError::PacketRead)?;
    if header.version() != 4 {
        return Err(RxError::BadVersion(header.version()));
    }

    let header_len = header.ihl() as usize * 4;
    if header_len != size_of::<Ipv4Header>() {
        // TODO:
        return Err(RxError::BadHeaderLength(header.ihl()));
    }

    let src = Ipv4Addr::from(header.src_addr);
    let remote = IpAddr::V4(src);
    let dst = Ipv4Addr::from(header.dst_addr);
    let protocol = header.protocol;

    // TODO: check dst address

    // truncate the packet to the IPv4 payload length. Ethernet may have added
    // padding for its minimum frame size.
    let ipv4_len: u16 = header.len.into();
    let payload_len = (ipv4_len as usize).saturating_sub(header_len);
    pkt.truncate(payload_len);

    match protocol {
        0x06 => {
            crate::tcp::handle_rx::<I>(devices, routes, sockets, pkt, remote, IpAddr::V4(dst))
                .map_err(RxError::Tcp)
        }
        protocol => Err(RxError::UnsupportedProtocol(protocol)),
    }
}
