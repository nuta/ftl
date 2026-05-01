use alloc::vec::Vec;

use crate::OutOfMemoryError;
use crate::device::InterfaceId;
use crate::ip::IpAddr;
use crate::ip::ipv4::IpCidr;
use crate::ip::ipv4::Ipv4Addr;
use crate::utils::VecExt;

pub struct Route {
    iface_id: InterfaceId,
    cidr: IpCidr,
    gateway: Option<Ipv4Addr>,
}

impl Route {
    pub fn new(iface_id: InterfaceId, cidr: IpCidr) -> Self {
        Self {
            iface_id,
            cidr,
            gateway: None,
        }
    }

    pub fn iface_id(&self) -> InterfaceId {
        self.iface_id
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
        self.routes.try_push(route)?;
        Ok(())
    }

    pub fn lookup(&self, addr: IpAddr) -> Option<(InterfaceId, IpAddr)> {
        for route in &self.routes {
            let IpAddr::V4(addr) = addr;
            let IpCidr::Ipv4(cidr) = route.cidr;
            if cidr.contains(addr) {
                let dest_addr = route.gateway.unwrap_or(addr);
                return Some((route.iface_id, IpAddr::V4(dest_addr)));
            }
        }

        None
    }
}
