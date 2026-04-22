use core::fmt;

use crate::Io;
use crate::endian::Ne;
use crate::packet::Packet;
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
struct EthernetHeader {
    dst_addr: MacAddr,
    src_addr: MacAddr,
    ether_type: Ne<u16>,
}

pub(crate) fn handle_rx<I: Io>(routes: &mut RouteTable<I::Device>, pkt: &mut Packet) {
    let header = pkt.read::<EthernetHeader>().unwrap();
    info!("Ethernet header: {:#?}", header);
    let ether_type: u16 = header.ether_type.into();
    match ether_type {
        0x0800 => {
            if let Err(err) = crate::ip::ipv4::handle_rx(pkt) {
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
