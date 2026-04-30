use core::fmt;

use crate::Device;
use crate::TcpIp;
use crate::endian::Ne;
use crate::io::Io;
use crate::ip::IpAddr;
use crate::packet;
use crate::packet::Packet;
use crate::packet::WriteableToPacket;
use crate::route::Route;

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
pub(crate) enum TxError {
    PacketWrite(
        #[expect(dead_code)]
        packet::ReserveError),
}

pub(crate) fn transmit<D: Device>(
    device: &mut D,
    route: &Route,
    ether_type: EtherType,
    pkt: &mut Packet,
    dst_addr: IpAddr,
) -> Result<(), TxError> {
    let dest_mac = match dst_addr {
        IpAddr::V4(addr) => {
            match route.arp_table().lookup(addr) {
                Some(mac) => *mac,
                None => {
                    // TODO: should we enqueue the packet to the ARP table?
                    todo!();
                }
            }
        }
    };

    let header = EthernetHeader {
        dst: dest_mac,
        src: *device.mac_addr(),
        ether_type: (ether_type as u16).into(),
    };

    pkt.write_front(header).map_err(TxError::PacketWrite)?;
    device.transmit(pkt);
    Ok(())
}

#[derive(Debug)]
pub enum RxError {
    PacketRead(packet::ReserveError),
    BadEthernetType(u16),
    Ipv4(crate::ip::ipv4::RxError),
    Arp(crate::arp::RxError),
}

pub(crate) fn handle_rx<I: Io>(tcpip: &mut TcpIp<I>, pkt: &mut Packet) -> Result<(), RxError> {
    let header = pkt.read::<EthernetHeader>().map_err(RxError::PacketRead)?;

    let ether_type: u16 = header.ether_type.into();
    match ether_type {
        0x0800 => crate::ip::ipv4::handle_rx::<I>(tcpip, pkt).map_err(RxError::Ipv4),
        0x0806 => crate::arp::handle_rx::<I>(tcpip, pkt).map_err(RxError::Arp),
        _ => Err(RxError::BadEthernetType(ether_type)),
    }
}
