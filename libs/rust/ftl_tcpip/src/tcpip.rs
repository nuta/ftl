use alloc::sync::Arc;
use core::fmt;

use crate::OutOfMemoryError;
use crate::interface::InterfaceMap;
use crate::interface::Interface;
use crate::interface::InterfaceId;
use crate::io::Io;
use crate::ip::IpAddr;
use crate::packet::Packet;
use crate::route::Route;
use crate::route::RouteTable;
use crate::socket::Endpoint;
use crate::socket::SocketMap;
use crate::tcp::TcpConn;
use crate::tcp::TcpListener;

pub struct TcpIp<I: Io> {
    pub(crate) interfaces: InterfaceMap<I::Device>,
    pub(crate) routes: RouteTable,
    pub(crate) sockets: SocketMap,
}

impl<I: Io> TcpIp<I> {
    pub fn new() -> Self {
        Self {
            interfaces: InterfaceMap::new(),
            routes: RouteTable::new(),
            sockets: SocketMap::new(),
        }
    }

    pub fn handle_rx(&mut self, pkt: &mut Packet) -> Result<(), crate::ethernet::RxError> {
        crate::ethernet::handle_rx::<I>(self, pkt)
    }

    pub fn add_device(&mut self, device: I::Device) -> Result<InterfaceId, OutOfMemoryError> {
        self.interfaces.add(device)
    }

    pub fn get_iface_mut(&mut self, id: InterfaceId) -> Option<&mut Interface<I::Device>> {
        self.interfaces.get_mut(id)
    }

    pub fn add_route(&mut self, route: Route) -> Result<(), OutOfMemoryError> {
        self.routes.add(route)
    }

    pub fn tcp_listen(
        &mut self,
        local: Endpoint,
    ) -> Result<TcpListenerHandle<I>, OutOfMemoryError> {
        self.sockets
            .create_tcp_listener(local)
            .map(TcpListenerHandle)
    }

    pub fn tcp_accept(
        &mut self,
        handle: TcpListenerHandle<I>,
        req: I::TcpAccept,
    ) -> TcpConnHandle<I> {
        let conn = handle.0.accept(self, req);
        TcpConnHandle(conn)
    }

    pub fn tcp_write(&mut self, handle: TcpConnHandle<I>, req: I::TcpWrite) {
        handle.0.write(self, req);
    }

    pub fn tcp_read(&mut self, handle: TcpConnHandle<I>, req: I::TcpRead) {
        handle.0.read(req);
    }

    pub fn tcp_close(&mut self, handle: TcpConnHandle<I>) {
        handle.0.close(self);
    }

    pub(crate) fn lookup_route(
        &mut self,
        addr: IpAddr,
    ) -> Option<(&mut Interface<I::Device>, IpAddr)> {
        if let Some((iface_id, dest_addr)) = self.routes.lookup(addr) {
            let iface = self.interfaces.get_mut(iface_id).unwrap();
            Some((iface, dest_addr))
        } else {
            None
        }
    }
}

pub struct TcpConnHandle<I: Io>(pub(crate) Arc<TcpConn<I>>);

pub struct TcpListenerHandle<I: Io>(pub(crate) Arc<TcpListener<I>>);

impl<I: Io> fmt::Debug for TcpConnHandle<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpConn").finish()
    }
}

impl<I: Io> fmt::Debug for TcpListenerHandle<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpListener").finish()
    }
}

impl<I: Io> Clone for TcpConnHandle<I> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<I: Io> Clone for TcpListenerHandle<I> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
