#![no_std]

extern crate alloc;

mod arp;
mod device;
mod dhcp;
mod mux;
mod nic;
mod packet;
mod rx;
mod tx;
mod udp;

pub use crate::device::DeviceId;
pub use crate::device::PollNotifier;
pub use crate::mux::NetMux;
pub use crate::mux::RxNotify;
pub use crate::nic::BufReader;
pub use crate::nic::BufWriter;
pub use crate::nic::NicId;
