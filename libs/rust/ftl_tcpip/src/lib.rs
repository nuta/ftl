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

pub fn handle_packet<AcceptR: tcp::AcceptRequest>(
    sockets: &SocketMap,
    routes: &RouteTable,
    packet: &[u8],
) {
    let fivetuple = todo!();
    let listener = sockets.get::<TcpListener<AcceptR>>(fivetuple);
}
