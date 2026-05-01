use alloc::sync::Arc;
use core::any::Any;

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

pub trait AnySocket: Any + Send + Sync {}

pub struct SocketMap {
    actives: HashMap<ActiveKey, Arc<dyn AnySocket>>,
    listeners: HashMap<ListenerKey, Arc<dyn AnySocket>>,
}

impl SocketMap {
    pub(crate) fn new() -> Self {
        Self {
            actives: HashMap::new(),
            listeners: HashMap::new(),
        }
    }

    pub(crate) fn get_active<T: AnySocket>(&self, key: &ActiveKey) -> Option<Arc<T>> {
        let any_socket = self.actives.get(key)?.clone() as Arc<dyn Any + Send + Sync>;
        let socket = any_socket.downcast::<T>().ok()?;
        Some(socket)
    }

    pub(crate) fn get_listener<T: AnySocket>(&self, key: &ListenerKey) -> Option<Arc<T>> {
        let any_socket = self.listeners.get(key)?.clone() as Arc<dyn Any + Send + Sync>;
        let socket = any_socket.downcast::<T>().ok()?;
        Some(socket)
    }

    pub(crate) fn establish_tcp_conn<I: Io>(
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
        self.actives.reserve_and_insert(key, conn.clone())?;
        Ok(())
    }

    pub(crate) fn handle_timeout<I: Io>(&mut self, now: I::Instant) -> Option<I::Instant> {
        let mut earliest: Option<I::Instant> = None;
        self.actives.retain(|key, socket| {
            // TODO: Replace Any with enum.
            let socket = socket.clone() as Arc<dyn Any + Send + Sync>;
            let conn = socket.downcast::<TcpConn<I>>().unwrap();
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

    pub(crate) fn create_tcp_listener<I: Io>(
        &mut self,
        local: Endpoint,
    ) -> Result<Arc<TcpListener<I>>, OutOfMemoryError> {
        let key = ListenerKey {
            local,
            protocol: Protocol::Tcp,
        };

        let socket = Arc::new(TcpListener::new(local.port));
        self.listeners.reserve_and_insert(key, socket.clone())?;
        Ok(socket)
    }
}
