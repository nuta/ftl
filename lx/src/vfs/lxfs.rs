use alloc::sync::Arc;

use ftl::quota::Quota;
use ftl::vmo::Vmo;

use crate::vfs::DirEntry;
use crate::vfs::INode;
use crate::vfs::StaticDir;
use crate::zone::Fork;

pub struct LxFs {
    _private: (),
}

impl LxFs {
    pub fn new(quota: Arc<Quota>, image: Vmo) -> StaticDir<1> {
        let fork_dir = StaticDir::new([DirEntry::new(
            b"fork",
            INode::File(Arc::new(Fork::new(quota, image))),
        )]);

        StaticDir::new([DirEntry::new(b"zone", INode::Dir(Arc::new(fork_dir)))])
    }
}
