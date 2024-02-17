#![no_std]

extern crate alloc;

// Hihg-level APIs.
pub mod device;
pub mod mainloop;
pub mod print;
pub mod sync;

// Kernel-provided primitives.
pub mod channel;
pub mod environ;
pub mod event_poll;
pub mod folio;

// Low-level APIs.
pub mod entrypoint;
pub mod handle;
pub mod syscall;

pub mod types {
    pub use ftl_types::*;
}

// FIXME:
pub use ftl_types::Message;
