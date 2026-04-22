use alloc::vec::Vec;

use crate::Device;
use crate::OutOfMemoryError;
use crate::arp::ArpTable;
use crate::ethernet::MacAddr;
use crate::ip::ipv4::Ipv4Addr;
use crate::ip::ipv4::NetMask;

pub struct Route<D: Device> {
    device: D,
    arp_table: ArpTable,
    ipv4_addr: Ipv4Addr,
    net_mask: NetMask,
    mac_addr: MacAddr,
}

impl<D: Device> Route<D> {
    pub fn new(device: D, ipv4_addr: Ipv4Addr, net_mask: NetMask, mac_addr: MacAddr) -> Self {
        Self {
            device,
            arp_table: ArpTable::new(),
            ipv4_addr,
            net_mask,
            mac_addr,
        }
    }

    pub fn device(&self) -> &D {
        &self.device
    }

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

pub struct RouteTable<D: Device> {
    routes: Vec<Route<D>>,
}

impl<D: Device> RouteTable<D> {
    pub const fn new() -> Self {
        Self { routes: Vec::new() }
    }

    pub fn add(&mut self, route: Route<D>) -> Result<(), OutOfMemoryError> {
        self.routes.try_reserve(1).map_err(|_| OutOfMemoryError)?;
        self.routes.push(route);
        Ok(())
    }

    pub fn lookup_by_dest_exact(&self, dest_addr: Ipv4Addr) -> Option<&Route<D>> {
        self.routes
            .iter()
            .find(|route| route.should_receive(dest_addr))
    }

    pub fn lookup_by_dest(&self, dest_addr: Ipv4Addr) -> Option<&Route<D>> {
        self.routes
            .iter()
            .find(|route| route.should_receive(dest_addr))
    }
}
