#![cfg_attr(not(feature = "std"), no_std)]
#![allow(unused)] // TODO:

use crate::route::RouteTable;
use crate::socket::SocketMap;
use crate::tcp::TcpListener;

extern crate alloc;

pub mod address;
pub mod ipv4;
pub mod route;
pub mod socket;
pub mod arp;
pub mod tcp;
mod utils;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutOfMemoryError;

pub trait Io: 'static {
    type TcpWrite: tcp::Write;
    type TcpRead: tcp::Read;
    type TcpAccept: tcp::Accept;
}

pub fn receive_packet<I: Io>(sockets: &SocketMap, routes: &RouteTable, packet: &[u8]) {
    let fivetuple = todo!();
    let listener = sockets.get::<TcpListener<I>>(fivetuple);
}
