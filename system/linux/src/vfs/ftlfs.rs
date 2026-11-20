use alloc::sync::Arc;

use ftl::quota::Quota;
use ftl::vmo::Vmo;

use crate::net::TcpIp;
use crate::vfs::DirEntry;
use crate::vfs::INode;
use crate::vfs::MemDir;
use crate::zone::Fork;

pub struct FtlFs {
    _private: (),
}

impl FtlFs {
    pub fn new(quota: Arc<Quota>, image: Vmo, network: Arc<TcpIp>) -> MemDir {
        let fork_dir = MemDir::with_entries([DirEntry::new(
            b"fork",
            INode::File(Arc::new(Fork::new(quota, image, network))),
        )]);

        MemDir::with_entries([DirEntry::new(b"zone", INode::Dir(Arc::new(fork_dir)))])
    }
}
