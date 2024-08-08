#![no_std]
#![feature(asm_const)]
#![feature(naked_functions)]
#![feature(arbitrary_self_types)]
#![feature(fn_align)]

extern crate alloc;

#[macro_use]
mod print;

pub mod boot;
pub mod cpuvar;

mod arch;
mod channel;
mod device_tree;
mod folio;
mod handle;
mod interrupt;
mod memory;
mod panic;
mod poll;
mod process;
mod ref_counted;
mod scheduler;
mod signal;
mod sleep;
mod spinlock;
mod syscall;
mod thread;
mod userboot;
mod utils;
