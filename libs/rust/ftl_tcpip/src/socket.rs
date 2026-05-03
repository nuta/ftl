use alloc::sync::Arc;

use hashbrown::HashMap;

use crate::OutOfMemoryError;
use crate::dhcp::DhcpClient;
use crate::io::Instant;
use crate::io::Io;
use crate::ip::IpAddr;
use crate::ip::Ipv4Addr;
use crate::tcp::TcpConn;
use crate::tcp::TcpListener;
use crate::tcp::TimeoutResult;
use crate::transport::Port;
use crate::udp::UdpSocket;
use crate::utils::HashMapExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Endpoint {
    pub addr: IpAddr,
    pub port: Port,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TcpConnKey {
    pub remote: Endpoint,
    pub local: Endpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TcpListenerKey {
    pub local: Endpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct UdpSocketKey {
    pub local: Endpoint,
}

pub struct SocketMap<I: Io> {
    actives: HashMap<TcpConnKey, Arc<TcpConn<I>>>,
    listeners: HashMap<TcpListenerKey, Arc<TcpListener<I>>>,
    udp_sockets: HashMap<UdpSocketKey, Arc<UdpSocket<I>>>,
    dhcp_clients: HashMap<UdpSocketKey, DhcpClient>,
}

impl<I: Io> SocketMap<I> {
    pub(crate) fn new() -> Self {
        Self {
            actives: HashMap::new(),
            listeners: HashMap::new(),
            udp_sockets: HashMap::new(),
            dhcp_clients: HashMap::new(),
        }
    }

    pub(crate) fn get_tcp_conn(
        &self,
        local: &Endpoint,
        remote: &Endpoint,
    ) -> Option<Arc<TcpConn<I>>> {
        self.actives
            .get(&TcpConnKey {
                local: *local,
                remote: *remote,
            })
            .cloned()
    }

    pub(crate) fn get_tcp_listener(&self, local: &Endpoint) -> Option<Arc<TcpListener<I>>> {
        let mut key = TcpListenerKey { local: *local };
        if let Some(socket) = self.listeners.get(&key) {
            return Some(socket.clone());
        }

        key.local.addr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
        self.listeners.get(&key).cloned()
    }

    pub(crate) fn establish_tcp_conn(
        &mut self,
        remote: Endpoint,
        local: Endpoint,
        conn: Arc<TcpConn<I>>,
    ) -> Result<(), OutOfMemoryError> {
        let key = TcpConnKey { remote, local };
        self.actives.reserve_and_insert(key, conn)?;
        Ok(())
    }

    pub(crate) fn handle_timeout(&mut self, now: &I::Instant) -> Option<I::Instant> {
        let mut earliest: Option<I::Instant> = None;
        self.actives.retain(|key, conn| {
            match conn.handle_timeout(&now) {
                TimeoutResult::Ok => true,
                TimeoutResult::ResetTimer(next) => {
                    let skip = matches!(earliest, Some(e) if e.is_before(&next));
                    if !skip {
                        earliest = Some(next);
                    }

                    true
                }
                TimeoutResult::Closed => {
                    trace!("destroying an socket: {:?}", key);
                    false
                }
            }
        });

        trace!(
            "{} active sockets, {} listener sockets",
            self.actives.len(),
            self.listeners.len()
        );

        earliest
    }

    pub(crate) fn create_tcp_listener(
        &mut self,
        local: Endpoint,
    ) -> Result<Arc<TcpListener<I>>, OutOfMemoryError> {
        let key = TcpListenerKey { local };
        let socket = Arc::new(TcpListener::new(local.port));
        self.listeners.reserve_and_insert(key, socket.clone())?;
        Ok(socket)
    }

    pub(crate) fn create_udp_socket(
        &mut self,
        local: Endpoint,
    ) -> Result<Arc<UdpSocket<I>>, OutOfMemoryError> {
        let key = UdpSocketKey { local };
        let socket = Arc::new(UdpSocket::<I>::new(local.port));
        self.udp_sockets.reserve_and_insert(key, socket.clone())?;
        Ok(socket)
    }

    pub(crate) fn get_udp_socket(&self, local: &Endpoint) -> Option<Arc<UdpSocket<I>>> {
        let mut key = UdpSocketKey { local: *local };
        if let Some(socket) = self.udp_sockets.get(&key) {
            return Some(socket.clone());
        }

        key.local.addr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
        self.udp_sockets.get(&key).cloned()
    }

    pub(crate) fn register_dhcp_client(
        &mut self,
        local: Endpoint,
        client: DhcpClient,
    ) -> Result<(), OutOfMemoryError> {
        let key = UdpSocketKey { local };
        self.dhcp_clients.reserve_and_insert(key, client)?;
        Ok(())
    }

    pub(crate) fn get_dhcp_client_mut(&mut self, local: &Endpoint) -> Option<&mut DhcpClient> {
        let key = UdpSocketKey { local: *local };
        self.dhcp_clients.get_mut(&key)
    }
}
