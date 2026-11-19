use core::ffi::CStr;

use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::errno::Errno;
use crate::types::size_t;

pub fn sys_readlink(
    current: &LxThread,
    path: *const u8,
    _buf: *mut u8,
    _bufsiz: size_t,
) -> Result<SyscallResult, Errno> {
    if path.is_null() {
        return Err(Errno::EFAULT);
    }

    let path = unsafe { CStr::from_ptr(path.cast()) }.to_bytes();

    // TODO: Support symlinks. Check the existence of the path and return
    // EINVAL for the time being.
    current.process().lookup_path(path, None)?;
    Err(Errno::EINVAL)
}
