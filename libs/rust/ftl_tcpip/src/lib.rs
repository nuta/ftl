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
pub mod tcp;

pub trait Io: 'static {
    type TcpWrite: tcp::WriteRequest;
    type TcpRead: tcp::ReadRequest;
    type TcpAccept: tcp::AcceptRequest;
}

pub fn handle_packet<I: Io>(
    sockets: &SocketMap,
    routes: &RouteTable,
    packet: &[u8],
) {
    let fivetuple = todo!();
    let listener = sockets.get::<TcpListener<I>>(fivetuple);
}
