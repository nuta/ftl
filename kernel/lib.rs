#![no_std]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(offset_of)]
#![feature(fn_align)]

extern crate alloc;

#[macro_use]
mod print;

pub mod boot;

mod arch;
mod folio;
mod handle;
mod memory;
mod spinlock;

#[cfg(target_family = "ftl")]
mod panic;
