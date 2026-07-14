#![no_std]

extern crate alloc;

use alloc::boxed::Box;

#[macro_use]
pub mod print;

pub mod error;
pub mod handle;
pub mod start;
pub mod thread;
pub mod upcall;
pub mod vmarea;
pub mod vmspace;

pub type Result<T> = core::result::Result<T, error::ErrorCode>;

pub struct Spec {
    pub name: &'static [u8],
    pub start: fn(),
}

pub fn start<R, F: Fn() -> R>(ctor: F) {
    // FIXME: Free the box when the server is stopped.
    // FIXME: Should we increment the server's ref count when registering a upcall?
    Box::leak(Box::new(ctor()));
}

#[cfg(not(feature = "kernel"))]
mod panic;

#[cfg(not(feature = "kernel"))]
pub mod allocator;
