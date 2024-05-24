#![no_std]
#![feature(asm_const)]
#![feature(effects)]
#![feature(const_trait_impl)]
#![feature(offset_of)]
#![feature(naked_functions)]

extern crate alloc;

#[macro_use]
mod print;

pub mod boot;
pub mod cpuvar;

mod arch;
mod memory;
mod panic;
mod scheduler;
mod spinlock;
mod thread;
