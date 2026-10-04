use alloc::sync::Arc;

use crate::types::errno::Errno;
use crate::vfs::Directory;
use crate::vfs::EmbeddedFile;
use crate::vfs::INode;

/// The contents of `/proc/version`.
const VERSION: &[u8] = concat!("FTL version ", env!("CARGO_PKG_VERSION"), "\n").as_bytes();

/// The proc pseudo file system (`/proc`).
pub struct ProcFs {
    _private: (),
}

impl ProcFs {
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Directory for ProcFs {
    fn lookup(&self, name: &[u8]) -> Result<INode, Errno> {
        match name {
            b"version" => Ok(INode::File(Arc::new(EmbeddedFile::new(VERSION)))),
            _ => Err(Errno::ENOENT),
        }
    }
}
