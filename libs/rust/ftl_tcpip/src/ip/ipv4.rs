use core::fmt;

use crate::ethernet::EtherType;
use crate::{Io, ethernet, transport};
use crate::device::{Device, DeviceMap};
use crate::endian::Ne;
use crate::ip::IpAddr;
use crate::packet::{Packet, WriteableToPacket};
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
}

#[derive(Debug)]
pub enum TxError {
    NoRoute,
    NoDevice,
    PacketWrite(packet::ReserveError),
    EthernetTx(ethernet::TxError),
}

pub(crate) fn transmit<I: Io>(
    devices: &mut DeviceMap<I::Device>,
    routes: &mut RouteTable,
    pkt: &mut Packet,
    dest_ip: Ipv4Addr,
    protocol: transport::Protocol,
) -> Result<(), TxError> {
    let Some(route) = routes.lookup_by_dest(IpAddr::V4(dest_ip)) else {
        return Err(TxError::NoRoute);
    };

    let Some(device) = devices.get_mut(route.device_id()) else {
        return Err(TxError::NoDevice);
    };

    let header = Ipv4Header {
        version_and_ihl: 4 << 4 | 5,
        tos: 0,
        len: (pkt.len() as u16).into(),
        id: 0.into(),
        flags_and_frag_offset: 0.into(),
        ttl: 64,
        protocol: protocol as u8,
        checksum: 0.into(),
        src_addr: route.ipv4_addr().into(),
        dst_addr: dest_ip.into(),
    };

    pkt.write_front(header).map_err(TxError::PacketWrite)?;
    ethernet::transmit(device, EtherType::Ipv4, pkt).map_err(TxError::EthernetTx)?;
    Ok(())
}

#[derive(Debug)]
pub enum RxError {
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
) -> Result<(), RxError> {
    let header = pkt.read::<Ipv4Header>().map_err(RxError::PacketRead)?;
    if header.version() != 4 {
        return Err(RxError::BadVersion(header.version()));
    }

    if header.ihl() != 5 {
        return Err(RxError::BadHeaderLength(header.ihl()));
    }

    let src = Ipv4Addr::from(header.src_addr);
    let remote = IpAddr::V4(src);
    let dst = Ipv4Addr::from(header.dst_addr);
    info!("IPv4 packet: src: {}, dst: {}", src, dst);

    // TODO: check dst address

    match header.protocol {
        0x06 => {
            if let Err(err) =
                crate::transport::tcp::handle_rx::<I>(devices, routes, sockets, pkt, remote)
            {
                warn!("bad TCP packet: {:?}", err);
            }
        }
        protocol => {
            return Err(RxError::UnsupportedProtocol(protocol));
        }
    }

    Ok(())
}
