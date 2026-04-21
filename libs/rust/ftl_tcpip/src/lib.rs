#![cfg_attr(not(feature = "std"), no_std)]
#![allow(unused)] // TODO:

use std::ops::DerefMut;

use crate::route::RouteTable;
use crate::socket::SocketMap;
use crate::transport::tcp;
use crate::transport::tcp::TcpListener;

extern crate alloc;

pub mod arp;
pub mod ip;
pub mod route;
pub mod socket;
pub mod transport;
mod utils;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutOfMemoryError;

pub trait Mutex<T>: Send + Sync {
    type Guard<'a>: DerefMut<Target=T> + 'a where Self: 'a;
    fn lock(&self) -> Self::Guard<'_>;
}


impl<T: Send> Mutex<T> for std::sync::Mutex<T> {
    type Guard<'a> = std::sync::MutexGuard<'a, T> where Self: 'a;

    fn lock(&self) -> Self::Guard<'_> {
        self.lock().unwrap()
    }
}

pub trait RwLockReadGuard<T>: AsRef<T> {}
pub trait RwLockWriteGuard<T>: AsMut<T> {}

pub trait RwLock<T>: Send + Sync {
    type ReadGuard: RwLockReadGuard<T>;
    type WriteGuard: RwLockWriteGuard<T>;
    fn read(&self) -> Self::ReadGuard;
    fn write(&self) -> Self::WriteGuard;
}

pub trait Io: 'static {
    type Mutex<T>: Mutex<T>;
    type TcpWrite: tcp::Write;
    type TcpRead: tcp::Read;
    type TcpAccept: tcp::Accept;
}

pub fn receive_packet<I: Io>(sockets: &SocketMap, routes: &RouteTable, packet: &[u8]) {
    let key = todo!();
    let listener = sockets.get_listener::<TcpListener<I>>(key);
}
