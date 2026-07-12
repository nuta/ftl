#![no_std]

#[macro_use]
pub mod print;

pub mod error;
pub mod handle;
pub mod start;
pub mod vmarea;
pub mod vmspace;

pub type Result<T> = core::result::Result<T, error::ErrorCode>;

pub struct Spec {
    pub name: &'static [u8],
    pub start: fn(),
}

pub fn start<R, F: Fn() -> R>(ctor: F) {
    ctor();
}

#[cfg(not(feature = "kernel"))]
mod panic;

#[cfg(not(feature = "kernel"))]
pub mod allocator;
