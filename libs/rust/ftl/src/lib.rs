#![no_std]

extern crate alloc;

pub use alloc::borrow;
pub use alloc::boxed;
pub use alloc::rc;

pub use ftl_macros::main;

#[macro_use]
pub mod print;

pub mod allocator;

pub mod arch;
mod panic;

pub mod collections;
pub mod driver;
pub mod error;
pub mod eventloop;
pub mod handle;
pub mod log;
pub mod message;
pub mod prelude;
pub mod process;
pub mod sink;
pub mod syscall;
pub mod thread;
pub mod time;
pub mod vmarea;
pub mod vmspace;

pub mod channel;
pub mod interrupt;
#[cfg(target_arch = "x86_64")]
pub mod pci;
