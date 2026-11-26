use core::ffi::CStr;

use super::SyscallResult;
use crate::thread::Thread;
use crate::types::errno::Errno;
use crate::types::off_t;
use crate::vfs::INode;

pub fn sys_truncate(current: &Thread, path: *const u8, len: off_t) -> Result<SyscallResult, Errno> {
    if path.is_null() {
        return Err(Errno::EFAULT);
    }

    let Ok(len) = usize::try_from(len) else {
        return Err(Errno::EINVAL);
    };

    let path = unsafe { CStr::from_ptr(path.cast()) }.to_bytes();
    let pnode = current.process().lookup_path(path, None)?;
    let INode::File(file) = pnode.inode() else {
        return Err(Errno::EISDIR);
    };

    file.truncate(len)?;
    Ok(SyscallResult::Done(0))
}
