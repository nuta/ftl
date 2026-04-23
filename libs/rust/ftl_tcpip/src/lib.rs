#![cfg_attr(not(feature = "std"), no_std)]
#![allow(unused)] // TODO:

use core::ops::Deref;
use core::ops::DerefMut;

use crate::device::Device;
use crate::packet::Packet;
use crate::route::RouteTable;
use crate::socket::SocketMap;
use crate::transport::tcp;
use crate::transport::tcp::TcpListener;

extern crate alloc;

#[macro_use]
extern crate log;

mod arp;
pub mod device;
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

pub trait Io: 'static {
    type Device: Device;
    type TcpWrite: tcp::Write;
    type TcpRead: tcp::Read;
    type TcpAccept: tcp::Accept;
}
