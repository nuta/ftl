#![no_std]

extern crate alloc;

// Commonly used FTL APIs.
pub mod prelude;

// Rust standard library like APIs.
pub mod collections;
pub mod print;
pub mod sync;

// Generic FTL APIs.
pub mod mainloop;

// Device driver API.
pub mod device;

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
