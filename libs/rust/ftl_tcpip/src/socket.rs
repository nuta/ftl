use alloc::sync::Arc;
use core::any::Any;

use hashbrown::HashMap;

use crate::OutOfMemoryError;
use crate::io::Io;
use crate::ip::IpAddr;
use crate::tcp::TcpListener;
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
    pub fn new() -> Self {
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

    pub(crate) fn tcp_establish<T: AnySocket>(
        &mut self,
        remote: Endpoint,
        local: Endpoint,
        socket: Arc<T>,
    ) -> Result<(), OutOfMemoryError> {
        let key = ActiveKey {
            remote,
            local,
            protocol: Protocol::Tcp,
        };
        self.actives.reserve_and_insert(key, socket.clone())?;
        Ok(())
    }

    pub fn tcp_listen<I: Io>(
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
