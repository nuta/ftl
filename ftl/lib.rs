#![no_std]

extern crate alloc;

mod channel;
mod error;
mod event_queue;
mod handle;

pub use channel::Channel;
pub use error::{Error, Result};
pub use event_queue::{Event, EventQueue, Interest, Ready};
pub use handle::Handle;
