use alloc::sync::Arc;

use ftl::quota::Quota;
use ftl::vmo::Vmo;

use crate::initfs::InitFsLoader;
use crate::net::TcpIp;
use crate::vfs::DevFs;
use crate::vfs::DirEntry;
use crate::vfs::EmbeddedFile;
use crate::vfs::INode;
use crate::vfs::LxFs;
use crate::vfs::MemDir;
use crate::vfs::ProcFs;

fn visit_initfs_dir(parent_dir: Option<&[u8]>) -> MemDir {
    let mut dir = MemDir::new();
    for file in InitFsLoader::new(&crate::INITFS.0) {
        let name = match parent_dir {
            Some(parent) => {
                let Some(rest) = file.name.strip_prefix(parent) else {
                    // The prefix does not match. Skip this entry.
                    continue;
                };

                let Some(name) = rest.strip_prefix(b"/") else {
                    // This is not a directory. SKip it.
                    continue;
                };

                name
            }
            None => file.name,
        };

        if name.is_empty() || name.contains(&b'/') {
            continue;
        }

        // Ignore top-level /dev, /lx, and /proc since we'll add them later.
        if parent_dir.is_none() && matches!(name, b"dev" | b"lx" | b"proc") {
            continue;
        }

        let inode = if file.is_dir {
            INode::Dir(Arc::new(visit_initfs_dir(Some(file.name))))
        } else {
            INode::File(Arc::new(EmbeddedFile::new(file.data)))
        };

        dir.add(DirEntry::new(name, inode));
    }

    dir
}

/// The root directory.
pub struct RootDir;

impl RootDir {
    pub fn new(quota: Arc<Quota>, image: Vmo, network: Arc<TcpIp>) -> INode {
        let mut root = visit_initfs_dir(None);
        root.add(DirEntry::new(b"dev", INode::Dir(Arc::new(DevFs::new()))));
        root.add(DirEntry::new(
            b"lx",
            INode::Dir(Arc::new(LxFs::new(quota, image, network))),
        ));
        root.add(DirEntry::new(b"proc", INode::Dir(Arc::new(ProcFs::new()))));
        INode::Dir(Arc::new(root))
    }
}
