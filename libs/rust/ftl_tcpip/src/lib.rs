#![cfg_attr(not(feature = "std"), no_std)]
#![allow(unused)] // TODO:

use core::ops::Deref;
use core::ops::DerefMut;

use crate::packet::Packet;
use crate::route::RouteTable;
use crate::socket::SocketMap;
use crate::transport::tcp;
use crate::transport::tcp::TcpListener;

extern crate alloc;

#[macro_use]
extern crate log;

mod arp;
mod endian;
pub mod ethernet;
pub mod ip;
pub mod packet;
pub mod route;
pub mod socket;
pub mod transport;
mod utils;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutOfMemoryError;

pub trait Device {
    fn transmit(&self, pkt: &mut Packet);
}

pub trait Io: 'static {
    type Device: Device;
    type TcpWrite: tcp::Write;
    type TcpRead: tcp::Read;
    type TcpAccept: tcp::Accept;
}

pub fn receive_packet<I: Io>(
    sockets: &mut SocketMap,
    routes: &mut RouteTable<I::Device>,
    pkt: &mut Packet,
) {
    trace!("received packet: {:02x?}", pkt.len());
    ethernet::handle_rx::<I>(routes, pkt);
    // let key = todo!();
    // let listener = sockets.get_listener::<TcpListener<I>>(key);
}
