use alloc::sync::Arc;

use crate::vfs::DirEntry;
use crate::vfs::EmbeddedFile;
use crate::vfs::INode;
use crate::vfs::StaticDir;

/// The contents of `/proc/version`.
const VERSION: &[u8] = concat!("FTL version ", env!("CARGO_PKG_VERSION"), "\n").as_bytes();

/// The proc pseudo file system (`/proc`).
pub struct ProcFs {
    _private: (),
}

impl ProcFs {
    pub fn new() -> StaticDir<1> {
        StaticDir::new([DirEntry::new(
            b"version",
            INode::File(Arc::new(EmbeddedFile::new(VERSION))),
        )])
    }
}
