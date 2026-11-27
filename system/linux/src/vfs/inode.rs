use alloc::sync::Arc;

use crate::types::errno::Errno;
use crate::types::sys::stat::S_IFDIR;
use crate::types::sys::stat::Stat;
use crate::vfs::Directory;
use crate::vfs::FileLike;
use crate::vfs::addr_as_ino;

#[derive(Clone)]
pub enum INode {
    Dir(Arc<dyn Directory>),
    File(Arc<dyn FileLike>),
}

impl INode {
    pub fn stat(&self) -> Result<Stat, Errno> {
        match self {
            INode::Dir(dir) => Ok(Stat::new(addr_as_ino(&**dir), S_IFDIR | 0o755, 0)),
            INode::File(file) => file.stat(),
        }
    }

    pub fn chmod(&self, mode: u32) -> Result<(), Errno> {
        match self {
            INode::Dir(dir) => dir.chmod(mode),
            INode::File(file) => file.chmod(mode),
        }
    }
}
