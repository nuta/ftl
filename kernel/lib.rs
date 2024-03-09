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
pub mod channel;
pub mod event_poll;
pub mod fiber;
pub mod folio;

mod autopilot;
mod backtrace;
mod lock;
mod memory;
mod panic;
mod scheduler;
