//! FTL standard library for applications.
//!
//! This is [`libstd`](https://doc.rust-lang.org/std/) in FTL. All components
//! except the kernel use this library.
//!
//! # APIs you should to know
//!
//! Here are the most frequently used APIs that you will encounter first:
//!
//! - [`info!`], [`warn!`], [`error!`], [`debug!`], [`trace!`]: Logging macros.
//! - [`Channel`](channel::Channel): A channel for inter-process communication.
//! - [`Environ`](environ::Environ): Environment variables in FTL.
//!
//! ## Server APIs
//!
//! Servers are long-running processes that provide services to clients, such as
//! device drivers, filesystem drivers, TCP/IP stack, and so on.
//!
//! - [`Mainloop`](mainloop::Mainloop): An event loop for applications.
//!
//! ## Device driver APIs
//!
//! In FTL, device drivers are servers with the hardware access. To control the
//! hardware conveniently, FTL provides the following APIs:
//!
//! - [`MappedFolio`](folio::MappedFolio): A physically-contiguous memory region for DMA/MMIO access.
//! - [`Interrupt`](interrupt::Interrupt): Hardware interrupt handling.
//! - `ftl_driver_utils::mmio`: Type-safe MMIO access.
//! - `ftl_driver_utils::DmaBufferPool`: The DMA buffer pool allocator.
//!
//! # Prelude
//!
//! The [`prelude`] module contains the most common types and traits that you'll
//! use in FTL programs. Here is an idiomatic way to import the prelude:
//!
//! ```
//! use ftl_api::prelude::*;
//! ```

#![no_std]
#![feature(start)]

extern crate alloc;

mod allocator;
mod arch;
mod panic;
mod start;

pub mod channel;
pub mod environ;
pub mod folio;
pub mod handle;
pub mod interrupt;
pub mod log;
pub mod mainloop;
pub mod poll;
pub mod prelude;
pub mod print;
pub mod signal;
pub mod syscall;
pub mod vmspace;

/// Embeds the code generated by `ftl_autogen` crate.
#[macro_export]
macro_rules! autogen {
    () => {
        include!(concat!(env!("OUT_DIR"), "/autogen.rs"));
    };
}

/// Synchronization primitives such as `Arc` and `Weak`.
pub mod sync {
    pub use alloc::sync::Arc;
    pub use alloc::sync::Weak;
}

/// Collections. `HashMap`, `HashSet`, `VecDeque`, `BinaryHeap`, and more.
pub mod collections {
    pub use alloc::collections::*;

    pub use hashbrown::hash_map;
    pub use hashbrown::hash_set;
    pub use hashbrown::HashMap;
    pub use hashbrown::HashSet;
}

/// FTL types.
pub use ftl_types as types;
