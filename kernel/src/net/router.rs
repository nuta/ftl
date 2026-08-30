use alloc::vec::Vec;

use ftl_types::error::ErrorCode;
use ftl_utils::spinlock::SpinLock;

use super::device::Device;
use super::packet::Ipv4Addr;
use crate::net::packet::NetMask;
use crate::shared_ref::SharedRef;

pub struct Route {
    device: SharedRef<Device>,
    our_ip: Ipv4Addr,
    netmask: NetMask,
    gateway_ip: Ipv4Addr,
    next_hop_ip: Ipv4Addr,
}

impl Route {
    pub fn new(
        device: SharedRef<Device>,
        our_ip: Ipv4Addr,
        netmask: NetMask,
        gateway_ip: Ipv4Addr,
        next_hop_ip: Ipv4Addr,
    ) -> Self {
        Self {
            device,
            our_ip,
            netmask,
            gateway_ip,
            next_hop_ip,
        }
    }

    pub fn device(&self) -> &SharedRef<Device> {
        &self.device
    }

    pub fn our_ip(&self) -> Ipv4Addr {
        self.our_ip
    }

    pub fn next_hop_ip(&self) -> Ipv4Addr {
        self.next_hop_ip
    }
}

pub struct Router {
    routes: SpinLock<Vec<SharedRef<Route>>>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            routes: SpinLock::new(Vec::new()),
        }
    }

    pub fn add_route(&self, route: SharedRef<Route>) -> Result<(), ErrorCode> {
        let mut routes = self.routes.lock();
        routes
            .try_reserve(1)
            .map_err(|_| ErrorCode::OUT_OF_MEMORY)?;
        routes.push(route);
        Ok(())
    }

    pub fn lookup(&self, dst_ip: Ipv4Addr) -> Option<SharedRef<Route>> {
        let routes = self.routes.lock();
        for route in routes.iter() {
            if route.netmask.contains(dst_ip) {
                return Some(route.clone());
            }
        }

        None
    }
}
