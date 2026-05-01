#![cfg_attr(not(feature = "std"), no_std)]
// #![allow(unused)] // TODO:

use crate::interface::Device;

extern crate alloc;

#[macro_use]
extern crate log;

mod arp;
mod checksum;
pub mod interface;
mod endian;
pub mod ethernet;
pub mod io;
pub mod ip;
pub mod packet;
pub mod route;
pub mod socket;
pub mod tcp;
mod tcpip;
pub mod transport;
mod utils;

pub use io::Io;
pub use tcpip::TcpConnHandle;
pub use tcpip::TcpIp;
pub use tcpip::TcpListenerHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutOfMemoryError;
