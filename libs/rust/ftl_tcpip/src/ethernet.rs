use core::fmt;

use crate::{endian::Ne, packet::Packet};

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
        write!(f, "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}", self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5])
    }
}

#[derive(Debug)]
#[repr(C)]
struct EthernetHeader {
    dst_addr: MacAddr,
    src_addr: MacAddr,
    ether_type: Ne<u16>,
}

pub(crate) fn handle_rx(pkt: &mut Packet) {
    let header = pkt.read::<EthernetHeader>().unwrap();
    info!("Ethernet header: {:#?}", header);
    let ether_type: u16 = header.ether_type.into();
    match ether_type {
        0x0806 => {
            crate::arp::handle_rx(pkt);
        }
        _ => {
            warn!("unsupported Ethernet type: {:#x}", ether_type);
        }
    }
}
