#![no_std]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(offset_of)]

extern crate alloc;

#[macro_use]
pub mod print;

pub mod arch;
pub mod boot;
pub mod handle;
pub mod result;
pub mod sync;
pub mod task;

mod allocator;
mod backtrace;
mod panic;
