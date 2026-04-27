use alloc::sync::Arc;
use core::any::Any;

use hashbrown::HashMap;
use hashbrown::hash_map::OccupiedError;

use crate::Io;
use crate::ip::IpAddr;
use crate::tcp::TcpListener;
use crate::tcp::{self};
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

#[derive(Debug)]
pub enum TryInsertError {
    Reserve(hashbrown::TryReserveError),
    AlreadyExists,
}

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

    pub(crate) fn insert_active<T: AnySocket>(
        &mut self,
        key: ActiveKey,
        socket: Arc<T>,
    ) -> Result<(), TryInsertError> {
        self.actives
            .try_reserve(1)
            .map_err(TryInsertError::Reserve)?;
        self.actives
            .try_insert(key, socket.clone())
            .map_err(|_| TryInsertError::AlreadyExists)?;
        Ok(())
    }

    pub fn tcp_listen<I: Io>(
        &mut self,
        local: Endpoint,
    ) -> Result<Arc<TcpListener<I>>, TryInsertError> {
        let key = ListenerKey {
            local,
            protocol: transport::Protocol::Tcp,
        };

        let socket = Arc::new(TcpListener::<I>::new(local.port));

        self.listeners
            .try_reserve(1)
            .map_err(TryInsertError::Reserve)?;
        self.listeners
            .try_insert(key, socket.clone())
            .map_err(|_| TryInsertError::AlreadyExists)?;

        Ok(socket)
    }
}
