use alloc::vec::Vec;

use crate::Device;
use crate::OutOfMemoryError;
use crate::arp::ArpTable;
use crate::device::DeviceId;
use crate::ethernet::MacAddr;
use crate::ip::ipv4::Ipv4Addr;
use crate::ip::ipv4::NetMask;

pub struct Route {
    device_id: DeviceId,
    arp_table: ArpTable,
    ipv4_addr: Ipv4Addr,
    net_mask: NetMask,
}

impl Route {
    pub fn new(device_id: DeviceId, ipv4_addr: Ipv4Addr, net_mask: NetMask) -> Self {
        Self {
            device_id,
            arp_table: ArpTable::new(),
            ipv4_addr,
            net_mask,
        }
    }

    pub fn device_id(&self) -> DeviceId {
        self.device_id
    }

    pub fn ipv4_addr(&self) -> Ipv4Addr {
        self.ipv4_addr
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

    pub fn add(&mut self, route: Route) -> Result<(), OutOfMemoryError> {
        self.routes.try_reserve(1).map_err(|_| OutOfMemoryError)?;
        self.routes.push(route);
        Ok(())
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
