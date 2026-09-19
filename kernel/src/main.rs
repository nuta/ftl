#![cfg_attr(target_os = "none", no_std)]
#![no_main]
#![feature(coerce_unsized)]
#![feature(unsize)]
#![feature(unsafe_cell_access)]
#![feature(arbitrary_self_types)]
#![feature(dispatch_from_dyn)]

extern crate alloc;

#[macro_use]
mod print;

mod address;
mod arch;
mod boot;
mod cpuvar;
mod driver;
mod handle;
mod hspace;
mod loader;
mod memory;
mod net;
mod panic;
mod poll;
mod random;
mod scheduler;
mod shared_ref;
mod syscall;
mod thread;
mod timer;
mod vmobject;
mod vmspace;
