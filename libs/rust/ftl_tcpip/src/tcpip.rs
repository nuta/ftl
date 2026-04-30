use alloc::sync::Arc;

use crate::OutOfMemoryError;
use crate::device::DeviceId;
use crate::device::DeviceMap;
use crate::io::Io;
use crate::packet::Packet;
use crate::route::Route;
use crate::route::RouteTable;
use crate::socket::Endpoint;
use crate::socket::SocketMap;
use crate::tcp::TcpListener;

pub struct TcpIp<I: Io> {
    pub(crate) devices: DeviceMap<I::Device>,
    pub(crate) routes: RouteTable,
    pub(crate) sockets: SocketMap,
}

impl<I: Io> TcpIp<I> {
    pub fn new() -> Self {
        Self {
            devices: DeviceMap::new(),
            routes: RouteTable::new(),
            sockets: SocketMap::new(),
        }
    }

    pub fn handle_rx(&mut self, pkt: &mut Packet) -> Result<(), crate::ethernet::RxError> {
        crate::ethernet::handle_rx::<I>(self, pkt)
    }

    pub fn add_device(&mut self, device: I::Device) -> Result<DeviceId, OutOfMemoryError> {
        self.devices.add(device)
    }

    pub fn add_route(&mut self, route: Route) -> Result<(), OutOfMemoryError> {
        self.routes.add(route)
    }

    pub fn tcp_listen(&mut self, local: Endpoint) -> Result<Arc<TcpListener<I>>, OutOfMemoryError> {
        self.sockets.tcp_listen(local)
    }
}
