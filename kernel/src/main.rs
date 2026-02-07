#![no_std]
#![no_main]
#![feature(unsize)]
#![feature(coerce_unsized)]
#![feature(unsafe_cell_access)]
#![feature(arbitrary_self_types)]

extern crate alloc;

#[macro_use]
mod print;

mod address;
mod arch;
mod boot;
mod channel;
mod cpuvar;
mod handle;
mod initfs;
mod interrupt;
mod isolation;
mod loader;
mod memory;
mod panic;
mod process;
mod scheduler;
mod shared_ref;
mod sink;
mod spinlock;
mod syscall;
mod thread;
mod timer;
