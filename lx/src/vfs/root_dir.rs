use alloc::sync::Arc;

use crate::types::errno::Errno;
use crate::vfs::Directory;
use crate::vfs::INode;
use crate::vfs::ProcFs;

/// The root directory.
pub struct RootDir {
    procfs: INode,
}

impl RootDir {
    pub fn new() -> INode {
        let procfs = INode::Dir(Arc::new(ProcFs::new()));
        INode::Dir(Arc::new(Self { procfs }))
    }
}

impl Directory for RootDir {
    fn lookup(&self, name: &[u8]) -> Result<INode, Errno> {
        // TODO: Support dynamic mount points
        match name {
            b"proc" => Ok(self.procfs.clone()),
            _ => Err(Errno::ENOENT),
        }
    }
}
