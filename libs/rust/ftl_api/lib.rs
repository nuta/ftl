#![no_std]
#![feature(start)]

extern crate alloc;

pub mod allocator;
pub mod arch;
pub mod channel;
pub mod environ;
pub mod folio;
pub mod handle;
pub mod init;
pub mod interrupt;
pub mod log;
pub mod mainloop;
pub mod panic;
pub mod poll;
pub mod prelude;
pub mod print;
pub mod signal;
pub mod syscall;
pub mod vmspace;

pub mod sync {
    pub use alloc::sync::Arc;
    pub use alloc::sync::Weak;
}

pub mod collections {
    pub use alloc::collections::*;

    pub use hashbrown::hash_map;
    pub use hashbrown::hash_set;
    pub use hashbrown::HashMap;
    pub use hashbrown::HashSet;
}

pub use ftl_api_macros::main;
pub use ftl_types as types;
