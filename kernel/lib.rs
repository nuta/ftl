#![no_std]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(offset_of)]
#![feature(fn_align)]

extern crate alloc;

#[macro_use]
pub mod print;

pub mod arch;
pub mod boot;
pub mod memory;
pub mod spinlock;

#[cfg(target_family = "ftl")]
mod panic;
