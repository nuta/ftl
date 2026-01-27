#![no_std]

#[macro_use]
pub mod print;

mod arch;
mod panic;
mod syscall;

#[cfg(target_arch = "x86_64")]
pub mod pci;
