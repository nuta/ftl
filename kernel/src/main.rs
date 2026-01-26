#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
mod print;

mod address;
mod arch;
mod boot;
mod cpuvar;
mod memory;
mod panic;
mod scheduler;
mod spinlock;
mod syscall;
mod thread;
