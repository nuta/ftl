use alloc::sync::Arc;

use crate::vfs::Directory;
use crate::vfs::FileLike;

#[derive(Clone)]
pub enum INode {
    Dir(Arc<dyn Directory>),
    File(Arc<dyn FileLike>),
}
