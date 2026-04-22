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

pub struct RouteTable {
    routes: Vec<Route>,
}

impl RouteTable {
    pub const fn new() -> Self {
        Self { routes: Vec::new() }
    }

    pub fn lookup(&self, dest_addr: Ipv4Addr) -> Option<&Route> {
        for route in &self.routes {
            if route.ipv4_addr == dest_addr {
                return Some(route);
            }

            if route.net_mask.contains(&route.ipv4_addr, &dest_addr) {
                return Some(route);
            }
        }

        None
    }
}
