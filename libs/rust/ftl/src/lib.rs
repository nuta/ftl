#![no_std]

extern crate alloc;

pub use alloc::rc;

#[macro_use]
pub mod print;

pub mod allocator;

mod arch;
mod panic;

pub mod application;
pub mod collections;
pub mod dmabuf;
pub mod error;
pub mod handle;
pub mod prelude;
pub mod sink;
pub mod syscall;

pub mod channel;
pub mod interrupt;
#[cfg(target_arch = "x86_64")]
pub mod pci;
