#![cfg_attr(not(feature = "std"), no_std)]
// #![allow(unused)] // TODO:

use crate::device::Device;

extern crate alloc;

#[macro_use]
extern crate log;

mod arp;
mod checksum;
pub mod device;
mod endian;
pub mod ethernet;
pub mod io;
pub mod ip;
pub mod packet;
pub mod route;
pub mod socket;
pub mod tcp;
pub mod transport;
mod utils;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutOfMemoryError;
