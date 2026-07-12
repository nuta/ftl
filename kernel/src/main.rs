#![cfg_attr(target_os = "none", no_std)]
#![no_main]
#![feature(coerce_unsized)]
#![feature(unsize)]
#![feature(unsafe_cell_access)]
#![feature(arbitrary_self_types)]
#![allow(dead_code)]

extern crate alloc;

#[macro_use]
mod print;

mod address;
mod arch;
mod boot;
mod cpuvar;
mod initfs;
mod loader;
mod memory;
mod panic;
mod scheduler;
mod server;
mod shared_ref;
mod syscall;
mod thread;
mod vmarea;
mod vmspace;
