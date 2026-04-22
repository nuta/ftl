use alloc::vec::Vec;

use crate::arp::ArpTable;

pub struct Route {
    arp_table: ArpTable,
}

pub struct RouteTable {
    routes: Vec<Route>,
}

impl RouteTable {
    pub const fn new() -> Self {
        Self { routes: Vec::new() }
    }
}
