use core::fmt;

use crate::Device;
use crate::Io;
use crate::endian::Ne;
use crate::packet;
use crate::packet::Packet;
use crate::packet::WriteableToPacket;
use crate::route::Route;
use crate::route::RouteTable;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct MacAddr([u8; 6]);

impl MacAddr {
    pub const fn new(addr: [u8; 6]) -> Self {
        Self(addr)
    }
}

impl fmt::Debug for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

#[derive(Debug)]
#[repr(C)]
pub(crate) struct EthernetHeader {
    dst: MacAddr,
    src: MacAddr,
    ether_type: Ne<u16>,
}

#[derive(Debug)]
#[repr(u16)]
pub(crate) enum EtherType {
    Ipv4 = 0x0800,
    Arp = 0x0806,
}

impl WriteableToPacket for EthernetHeader {}

#[derive(Debug)]
pub enum TxError {
    PacketWrite(packet::ReserveError),
}

pub(crate) fn transmit<D: Device>(
    route: &Route<D>,
    ether_type: EtherType,
    dest_mac: MacAddr,
    pkt: &mut Packet,
) -> Result<(), TxError> {
    let header = EthernetHeader {
        dst: dest_mac,
        src: route.mac_addr(),
        ether_type: (ether_type as u16).into(),
    };

    pkt.write_front(header).map_err(TxError::PacketWrite)?;
    route.device().transmit(pkt);
    Ok(())
}

pub(crate) fn handle_rx<I: Io>(routes: &mut RouteTable<I::Device>, pkt: &mut Packet) {
    let header = pkt.read::<EthernetHeader>().unwrap();
    info!("Ethernet header: {:#?}", header);
    let ether_type: u16 = header.ether_type.into();
    match ether_type {
        0x0800 => {
            if let Err(err) = crate::ip::ipv4::handle_rx::<I>(routes, pkt) {
                warn!("bad IPv4 packet: {:?}", err);
            }
        }
        0x0806 => {
            if let Err(err) = crate::arp::handle_rx::<I>(routes, pkt) {
                warn!("bad ARP packet: {:?}", err);
            }
        }
        _ => {
            warn!("unsupported Ethernet type: {:#x}", ether_type);
        }
    }
}
