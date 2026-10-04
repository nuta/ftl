use core::ffi::CStr;

use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::vfs::INode;

pub fn sys_open(current: &LxThread, path: *const u8, flags: c_int) -> Result<SyscallResult, Errno> {
    if path.is_null() {
        return Err(Errno::EFAULT);
    }

    let path = unsafe { CStr::from_ptr(path.cast()) }.to_bytes();
    if path.is_empty() {
        return Err(Errno::ENOENT);
    }

    let process = current.process();
    let pnode = process.lookup_path(path)?;

    let file = match pnode.inode() {
        INode::File(file) => file.clone(),
        INode::Dir(_) => return Err(Errno::EISDIR),
    };

    let fd = process.fd_table().lock().insert(file.clone(), flags)?;
    Ok(SyscallResult::Done(fd as c_long))
}
