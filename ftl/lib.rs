#![no_std]

extern crate alloc;

mod error;
mod handle;

pub mod channel;
pub mod event_queue;

pub use error::{Error, Result};
pub use handle::Handle;
