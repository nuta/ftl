use crate::types::errno::Errno;
use crate::vfs::INode;

pub trait Directory: Send + Sync {
    fn lookup(&self, name: &[u8]) -> Result<INode, Errno>;
}
