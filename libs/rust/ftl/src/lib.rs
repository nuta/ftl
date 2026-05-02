#![no_std]

extern crate alloc;

#[macro_use]
extern crate log as log_crate;

pub use alloc::borrow;
pub use alloc::boxed;
pub use alloc::rc;
pub use alloc::string;

pub use ftl_macros::main;

#[macro_use]
pub mod print;

pub mod allocator;

pub mod arch;
mod message;
mod panic;

pub mod aio;
pub mod channel;
pub mod collections;
pub mod driver;
pub mod error;
pub mod handle;
pub mod interrupt;
pub mod log;
pub mod prelude;
pub mod process;
pub mod sink;
pub mod sync;
pub mod syscall;
pub mod thread;
pub mod time;
pub mod vmarea;
pub mod vmspace;

#[cfg(target_arch = "x86_64")]
pub mod pci;
