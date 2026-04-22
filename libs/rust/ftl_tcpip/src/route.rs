use alloc::vec::Vec;

use crate::arp::ArpTable;
use crate::ethernet::MacAddr;
use crate::ip::ipv4::Ipv4Addr;
use crate::ip::ipv4::NetMask;

pub struct Route {
    arp_table: ArpTable,
    ipv4_addr: Ipv4Addr,
    net_mask: NetMask,
    mac_addr: MacAddr,
}

impl Route {
    pub fn mac_addr(&self) -> MacAddr {
        self.mac_addr
    }

    fn should_receive_exact(&self, dest_addr: Ipv4Addr) -> bool {
        self.ipv4_addr == dest_addr
    }

    fn should_receive(&self, dest_addr: Ipv4Addr) -> bool {
        self.ipv4_addr == dest_addr || self.net_mask.contains(&self.ipv4_addr, &dest_addr)
    }
}

pub struct RouteTable {
    routes: Vec<Route>,
}

impl RouteTable {
    pub const fn new() -> Self {
        Self { routes: Vec::new() }
    }

    pub fn lookup_by_dest_exact(&self, dest_addr: Ipv4Addr) -> Option<&Route> {
        self.routes
            .iter()
            .find(|route| route.should_receive(dest_addr))
    }

    pub fn lookup_by_dest(&self, dest_addr: Ipv4Addr) -> Option<&Route> {
        self.routes
            .iter()
            .find(|route| route.should_receive(dest_addr))
    }
}
