#![no_std]
#![no_main]
#![feature(unsize)]
#![feature(coerce_unsized)]
#![feature(unsafe_cell_access)]

extern crate alloc;

#[macro_use]
mod print;

mod address;
mod arch;
mod boot;
mod cpuvar;
mod initfs;
mod isolation;
mod loader;
mod memory;
mod panic;
mod scheduler;
mod shared_ref;
mod spinlock;
mod syscall;
mod thread;
