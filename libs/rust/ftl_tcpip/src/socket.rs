use alloc::sync::Arc;

use hashbrown::HashMap;

use crate::OutOfMemoryError;
use crate::io::Instant;
use crate::io::Io;
use crate::ip::IpAddr;
use crate::tcp::TcpConn;
use crate::tcp::TcpListener;
use crate::tcp::TimeoutResult;
use crate::transport::Port;
use crate::transport::Protocol;
use crate::transport::{self};
use crate::utils::HashMapExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Endpoint {
    pub addr: IpAddr,
    pub port: Port,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActiveKey {
    pub remote: Endpoint,
    pub local: Endpoint,
    pub protocol: transport::Protocol,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ListenerKey {
    pub local: Endpoint,
    pub protocol: transport::Protocol,
}

pub(crate) enum AnySocket<I: Io> {
    TcpConn(Arc<TcpConn<I>>),
    TcpListener(Arc<TcpListener<I>>),
}

pub struct SocketMap<I: Io> {
    actives: HashMap<ActiveKey, AnySocket<I>>,
    listeners: HashMap<ListenerKey, AnySocket<I>>,
}

impl<I: Io> SocketMap<I> {
    pub(crate) fn new() -> Self {
        Self {
            actives: HashMap::new(),
            listeners: HashMap::new(),
        }
    }

    pub(crate) fn get_active(&self, key: &ActiveKey) -> Option<Arc<TcpConn<I>>> {
        let any_socket = self.actives.get(key)?;
        match any_socket {
            AnySocket::TcpConn(socket) => Some(socket.clone()),
            _ => None,
        }
    }

    pub(crate) fn get_listener(&self, key: &ListenerKey) -> Option<Arc<TcpListener<I>>> {
        let any_socket = self.listeners.get(key)?;
        match any_socket {
            AnySocket::TcpListener(socket) => Some(socket.clone()),
            _ => None,
        }
    }

    pub(crate) fn establish_tcp_conn(
        &mut self,
        remote: Endpoint,
        local: Endpoint,
        conn: Arc<TcpConn<I>>,
    ) -> Result<(), OutOfMemoryError> {
        let key = ActiveKey {
            remote,
            local,
            protocol: Protocol::Tcp,
        };
        self.actives
            .reserve_and_insert(key, AnySocket::TcpConn(conn))?;
        Ok(())
    }

    pub(crate) fn handle_timeout(&mut self, now: &I::Instant) -> Option<I::Instant> {
        let mut earliest: Option<I::Instant> = None;
        self.actives.retain(|key, socket| {
            // TODO: Replace Any with enum.
            match socket {
                AnySocket::TcpConn(conn) => {
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
                }
                AnySocket::TcpListener(_) => {
                    true
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

    pub(crate) fn create_tcp_listener(&mut self,
        local: Endpoint,
    ) -> Result<Arc<TcpListener<I>>, OutOfMemoryError> {
        let key = ListenerKey {
            local,
            protocol: Protocol::Tcp,
        };

        let socket = Arc::new(TcpListener::new(local.port));
        self.listeners.reserve_and_insert(key, AnySocket::TcpListener(socket.clone()))?;
        Ok(socket)
    }
}
