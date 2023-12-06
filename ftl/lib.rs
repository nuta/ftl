// #![no_std]

extern crate alloc;

#[macro_use]
pub mod logger;

mod error;
mod handle;
mod poll;

pub mod channel;
pub mod event_queue;

pub use error::{Error, Result};
pub use handle::Handle;
