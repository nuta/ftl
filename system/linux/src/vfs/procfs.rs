use alloc::sync::Arc;

use crate::vfs::DirEntry;
use crate::vfs::EmbeddedFile;
use crate::vfs::INode;
use crate::vfs::MemDir;

/// The contents of `/proc/version`.
const VERSION: &[u8] = concat!("FTL version ", env!("CARGO_PKG_VERSION"), "\n").as_bytes();

const MOUNTS: &[u8] = b"rootfs / rootfs ro 0 0\n\
    devfs /dev devfs rw 0 0\n\
    ftlfs /ftl ftlfs rw 0 0\n\
    proc /proc proc rw 0 0\n";

/// The proc pseudo file system (`/proc`).
pub struct ProcFs {
    _private: (),
}

impl ProcFs {
    pub fn new() -> MemDir {
        MemDir::with_entries([
            DirEntry::new(b"mounts", INode::File(Arc::new(EmbeddedFile::new(MOUNTS)))),
            DirEntry::new(
                b"version",
                INode::File(Arc::new(EmbeddedFile::new(VERSION))),
            ),
        ])
    }
}
