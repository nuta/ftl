use alloc::sync::Arc;
use core::any::Any;

use crate::io::InsertError;
use crate::io::Io;
use crate::io::Map;
use crate::ip::IpAddr;
use crate::tcp::TcpListener;
use crate::transport::Port;
use crate::transport::{self};

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

pub struct SocketMap<I: Io> {
    actives: I::Map<ActiveKey, Arc<dyn AnySocket>>,
    listeners: I::Map<ListenerKey, Arc<dyn AnySocket>>,
}

impl<I: Io> SocketMap<I> {
    pub fn new() -> Self {
        Self {
            actives: I::Map::new(),
            listeners: I::Map::new(),
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

    pub(crate) fn insert_active<T: AnySocket>(
        &mut self,
        key: ActiveKey,
        socket: Arc<T>,
    ) -> Result<(), InsertError> {
        self.actives.insert(key, socket.clone())
    }

    pub fn tcp_listen(&mut self, local: Endpoint) -> Result<Arc<TcpListener<I>>, InsertError> {
        let key = ListenerKey {
            local,
            protocol: transport::Protocol::Tcp,
        };

        let socket = Arc::new(TcpListener::<I>::new(local.port));
        self.listeners.insert(key, socket.clone())?;
        Ok(socket)
    }
}
