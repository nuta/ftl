use alloc::sync::Arc;
use core::any::Any;

use hashbrown::HashMap;
use hashbrown::hash_map::OccupiedError;

use crate::Io;
use crate::address::IpAddr;
use crate::tcp::TcpListener;
use crate::tcp::{self};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Endpoint {
    addr: IpAddr,
    port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TransportProtocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct FiveTuple {
    remote: Option<Endpoint>,
    local: Option<Endpoint>,
    protocol: TransportProtocol,
}

pub trait AnySocket: Any + Send + Sync {}

#[derive(Debug)]
pub enum TryInsertError {
    Reserve(hashbrown::TryReserveError),
    AlreadyExists,
}

pub struct SocketMap {
    inner: HashMap<FiveTuple, Arc<dyn AnySocket>>,
}

impl SocketMap {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    fn try_insert(
        &mut self,
        five_tuple: FiveTuple,
        socket: Arc<dyn AnySocket>,
    ) -> Result<(), TryInsertError> {
        self.inner.try_reserve(1).map_err(TryInsertError::Reserve)?;
        self.inner
            .try_insert(five_tuple, socket)
            .map_err(|_| TryInsertError::AlreadyExists)?;
        Ok(())
    }

    pub(crate) fn get<T: AnySocket>(&self, five_tuple: FiveTuple) -> Option<Arc<T>> {
        let any_socket = self.inner.get(&five_tuple)?.clone() as Arc<dyn Any + Send + Sync>;
        let socket = any_socket.downcast::<T>().ok()?;
        Some(socket)
    }

    pub fn tcp_listen<I: Io>(
        &mut self,
        local: Endpoint,
    ) -> Result<Arc<TcpListener<I>>, TryInsertError> {
        let five_tuple = FiveTuple {
            remote: None,
            local: Some(local),
            protocol: TransportProtocol::Tcp,
        };

        let socket = Arc::new(TcpListener::<I>::new());
        self.try_insert(five_tuple, socket.clone())?;
        Ok(socket)
    }
}
