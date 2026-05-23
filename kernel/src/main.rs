#![no_std]
#![no_main]
#![feature(coerce_unsized)]
#![feature(unsize)]

extern crate alloc;

#[macro_use]
mod print;

mod address;
mod arch;
mod boot;
mod memory;
mod panic;
mod shared_ref;
mod spinlock;

#[cfg(not(target_os = "none"))]
pub fn main() {
    crate::arch::host::main();
}
