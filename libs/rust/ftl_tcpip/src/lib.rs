#![cfg_attr(not(feature = "std"), no_std)]
#![allow(unused)] // TODO:

use core::ops::Deref;
use core::ops::DerefMut;

use crate::route::RouteTable;
use crate::socket::SocketMap;
use crate::transport::tcp;
use crate::transport::tcp::TcpListener;

extern crate alloc;

#[macro_use]
extern crate log;

mod arp;
pub mod ip;
pub mod route;
pub mod socket;
pub mod transport;
mod utils;
mod ethernet;
mod packet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutOfMemoryError;

pub trait Io: 'static {
    type TcpWrite: tcp::Write;
    type TcpRead: tcp::Read;
    type TcpAccept: tcp::Accept;
}

pub fn receive_packet<I: Io>(sockets: &SocketMap, routes: &RouteTable, packet: &[u8]) {
    trace!("received packet: {:02x?}", packet);
    // let key = todo!();
    // let listener = sockets.get_listener::<TcpListener<I>>(key);
}
