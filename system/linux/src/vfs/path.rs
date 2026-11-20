use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::types::errno::Errno;
use crate::vfs::INode;

/// A resolved path component, called a "path node", or "pnode".
///
/// For example, when you open `/home/user/hello.txt`, each path component,
/// `/home`, `user`, and `hello.txt`, has a separate pnode.
///
/// This is for fast path traversal, and to make `..` stable, even after the
/// parent directory is renamed.
pub struct PathNode {
    /// The parent pnode (that is "..").
    parent_dir: Option<Arc<PathNode>>,
    name: Vec<u8>,
    inode: INode,
    // TODO: Cache child pnodes to avoid creating new ones on each lookup,
    // but ... how should we invalidate them?
}

impl PathNode {
    pub fn root_dir(root_dir: INode) -> Arc<Self> {
        Arc::new(Self {
            parent_dir: None,
            name: Vec::new(),
            inode: root_dir,
        })
    }

    pub fn inode(&self) -> &INode {
        &self.inode
    }

    /// Resolves a path relative to this pnode.
    pub fn lookup(self: &Arc<Self>, path: &[u8]) -> Result<Arc<Self>, Errno> {
        let mut current = self.clone();
        for name in path.split(|&b| b == b'/') {
            // Are we in a directory?
            let INode::Dir(dir) = &current.inode else {
                return Err(Errno::ENOTDIR);
            };

            // Skip the empty name (e.g. "/foo//bar") or the current directory
            // (e.g. "/foo/.").
            if name.is_empty() || name == b"." {
                continue;
            }

            // Go to the parent directory.
            if name == b".." {
                // If None (the root directory), stay at the root. "/../etc"
                // is still valid and resolves to "/etc".
                if let Some(parent) = &current.parent_dir {
                    current = parent.clone();
                }

                continue;
            }

            // Look up the next path component.
            let inode = dir.lookup(name)?;
            current = Arc::new(Self {
                parent_dir: Some(current.clone()),
                name: name.into(),
                inode,
            });
        }

        Ok(current)
    }

    /// Writes the absolute path to `buf`.
    pub fn absolute_path(&self, buf: &mut [u8]) -> Result<usize, Errno> {
        // The root directory.
        if self.parent_dir.is_none() {
            let path = b"/\0";
            if path.len() > buf.len() {
                return Err(Errno::ERANGE);
            }

            buf[..path.len()].copy_from_slice(path);
            return Ok(path.len());
        }

        // Calculate the total length of the path.
        let mut len = 1;
        let mut current = self;
        while let Some(parent) = &current.parent_dir {
            // The name + the slash.
            len += current.name.len() + 1;
            current = parent;
        }

        if len > buf.len() {
            return Err(Errno::ERANGE);
        }

        // Write the null terminator.
        let mut end = len - 1;
        buf[end] = 0;

        // Write path components, from the end to the start.
        let mut current = self;
        while let Some(parent) = &current.parent_dir {
            let start = end - current.name.len();
            buf[start..end].copy_from_slice(&current.name);
            buf[start - 1] = b'/';
            end = start - 1;
            current = parent;
        }

        Ok(len)
    }
}
