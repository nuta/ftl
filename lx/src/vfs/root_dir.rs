use alloc::sync::Arc;

use crate::vfs::DevFs;
use crate::vfs::DirEntry;
use crate::vfs::INode;
use crate::vfs::ProcFs;
use crate::vfs::StaticDir;

/// The root directory.
pub struct RootDir;

impl RootDir {
    pub fn new() -> INode {
        // TODO: Support dynamic mount points
        INode::Dir(Arc::new(StaticDir::new([
            DirEntry::new(b"dev", INode::Dir(Arc::new(DevFs::new()))),
            DirEntry::new(b"proc", INode::Dir(Arc::new(ProcFs::new()))),
        ])))
    }
}
