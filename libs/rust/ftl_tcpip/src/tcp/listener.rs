use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;

use crate::Io;
use crate::device::DeviceMap;
use crate::ip::IpAddr;
use crate::ip::ipv4::Ipv4Addr;
use crate::packet::Packet;
use crate::route::RouteTable;
use crate::socket::ActiveKey;
use crate::socket::AnySocket;
use crate::socket::Endpoint;
use crate::socket::SocketMap;
use crate::tcp::TcpConn;
use crate::tcp::header::TcpFlags;
use crate::tcp::rx::RxHeader;
use crate::transport::Port;
use crate::transport::Protocol;

struct SynReceived {
    remote_ip: IpAddr,
    remote_port: Port,
    local_iss: u32,
    remote_rcv_nxt: u32,
}

struct Mutable<I: Io> {
    syn_received: Vec<SynReceived>,
    pending_accepts: VecDeque<I::TcpAccept>,
}

#[derive(Debug)]
pub enum AcceptError {}

pub struct TcpListener<I: Io> {
    local_port: Port,
    mutable: spin::Mutex<Mutable<I>>,
}

impl<I: Io> TcpListener<I> {
    pub(crate) fn new(local_port: Port) -> Self {
        Self {
            local_port,
            mutable: spin::Mutex::new(Mutable {
                syn_received: Vec::new(),
                pending_accepts: VecDeque::new(),
            }),
        }
    }

    pub fn accept(&self, request: I::TcpAccept) -> Result<Arc<TcpConn<I>>, AcceptError> {
        todo!()
    }

    fn start_handshake(
        self: &Arc<Self>,
        devices: &mut DeviceMap<I::Device>,
        routes: &mut RouteTable,
        sockets: &mut SocketMap,
        rx: RxHeader,
    ) {
        todo!()
    }

    fn finish_handshake(
        self: &Arc<Self>,
        devices: &mut DeviceMap<I::Device>,
        routes: &mut RouteTable,
        sockets: &mut SocketMap,
        rx: RxHeader,
    ) {
        todo!()
    }

    pub(super) fn handle_rx(
        self: &Arc<Self>,
        devices: &mut DeviceMap<I::Device>,
        routes: &mut RouteTable,
        sockets: &mut SocketMap,
        rx: RxHeader,
        payload: &mut Packet,
    ) {
        match rx.flags {
            TcpFlags::SYN => {
                self.start_handshake(devices, routes, sockets, rx);
            }
            TcpFlags::ACK | (TcpFlags::ACK | TcpFlags::PSH) => {
                self.finish_handshake(devices, routes, sockets, rx);
            }
            _ => {
                debug!("TCP: unexpected flags: {:?}", rx.flags);
                // TODO: Send an RST packet.
            }
        }
    }
}

impl<I: Io> AnySocket for TcpListener<I> {}

impl<I: Io> fmt::Debug for TcpListener<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpListener").finish()
    }
}
