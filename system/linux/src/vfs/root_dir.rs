use alloc::sync::Arc;

use ftl::quota::Quota;
use ftl::vmo::Vmo;

use crate::initfs::InitFsLoader;
use crate::net::TcpIp;
use crate::vfs::DevFs;
use crate::vfs::Directory;
use crate::vfs::FtlFs;
use crate::vfs::INode;
use crate::vfs::PathNode;
use crate::vfs::ProcFs;
use crate::vfs::TmpDir;
use crate::wait_queue::Sleep;

fn populate_from_initfs(root_dir: &Arc<TmpDir>) {
    let root = PathNode::root_dir(INode::Dir(root_dir.clone()));
    for file in InitFsLoader::new(&crate::INITFS.0) {
        let top_level = file.name.split(|&b| b == b'/').next().unwrap();
        // Ignore top-level /dev, /ftl, and /proc since we'll add them later.
        if matches!(top_level, b"" | b"." | b"dev" | b"ftl" | b"proc") {
            continue;
        }

        let (parent_path, name) = match file.name.iter().rposition(|&b| b == b'/') {
            Some(pos) => (&file.name[..pos], &file.name[pos + 1..]),
            None => (&b""[..], file.name),
        };

        let parent = root.lookup(parent_path).expect("initfs: parent not found");
        let INode::Dir(parent_dir) = parent.inode() else {
            panic!("initfs: parent is not a directory");
        };

        if file.is_dir {
            parent_dir
                .mkdir(name, file.mode)
                .expect("initfs: failed to mkdir");
        } else {
            let new_file = parent_dir.create(name, file.mode).unwrap();
            new_file
                .write(file.data, 0, false, Sleep::Uninterruptible)
                .unwrap();
        }
    }
}

/// Builds the root directory.
pub fn build_root_dir(quota: Arc<Quota>, image: Vmo, network: Arc<TcpIp>) -> INode {
    let root_dir = Arc::new(TmpDir::new(0o755));
    populate_from_initfs(&root_dir);

    let devfs = DevFs::new();
    let procfs = ProcFs::new();
    let ftlfs = FtlFs::new(quota, image, network);
    root_dir.add(b"proc", INode::Dir(Arc::new(procfs))).unwrap();
    root_dir.add(b"dev", INode::Dir(Arc::new(devfs))).unwrap();
    root_dir.add(b"ftl", INode::Dir(Arc::new(ftlfs))).unwrap();
    root_dir.mkdir(b"tmp", 0o1777).unwrap();

    INode::Dir(root_dir)
}
