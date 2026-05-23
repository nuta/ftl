#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
mod print;

mod address;
mod arch;
mod boot;
mod memory;
mod panic;
mod spinlock;

#[cfg(not(target_os = "none"))]
pub fn main() {
    crate::arch::host::main();
}
