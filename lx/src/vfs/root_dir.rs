use alloc::sync::Arc;

use ftl::quota::Quota;
use ftl::vmo::Vmo;

use crate::net::TcpIp;
use crate::vfs::DevFs;
use crate::vfs::DirEntry;
use crate::vfs::INode;
use crate::vfs::LxFs;
use crate::vfs::ProcFs;
use crate::vfs::StaticDir;

/// The root directory.
pub struct RootDir;

impl RootDir {
    pub fn new(quota: Arc<Quota>, image: Vmo, network: Arc<TcpIp>) -> INode {
        // TODO: Support dynamic mount points
        INode::Dir(Arc::new(StaticDir::new([
            DirEntry::new(b"dev", INode::Dir(Arc::new(DevFs::new()))),
            DirEntry::new(
                b"lx",
                INode::Dir(Arc::new(LxFs::new(quota, image, network))),
            ),
            DirEntry::new(b"proc", INode::Dir(Arc::new(ProcFs::new()))),
        ])))
    }
}
