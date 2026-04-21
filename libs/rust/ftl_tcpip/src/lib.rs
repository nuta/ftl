#![cfg_attr(not(feature = "std"), no_std)]
#![allow(unused)] // TODO:

use core::ops::{Deref, DerefMut};

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

pub trait Mutex<T: ?Sized>: Send + Sync {
    type Guard<'a>: Deref<Target=T> + DerefMut<Target=T> + 'a where Self: 'a;
    fn lock(&self) -> Self::Guard<'_>;
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
