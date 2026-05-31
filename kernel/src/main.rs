#![no_std]
#![no_main]
#![feature(coerce_unsized)]
#![feature(unsize)]
#![feature(unsafe_cell_access)]
#![feature(arbitrary_self_types)]

extern crate alloc;

#[macro_use]
mod print;

mod address;
mod arch;
mod boot;
mod cpuvar;
mod error;
mod loader;
mod memory;
mod panic;
mod scheduler;
mod shared_ref;
mod spinlock;
mod syscall;
mod thread;
mod vmarea;
mod vmspace;

#[cfg(not(target_os = "none"))]
pub fn main() {
    crate::arch::host::main();
}
