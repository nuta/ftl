use core::ffi::CStr;

use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::errno::Errno;

pub fn sys_chdir(current: &LxThread, path: *const u8) -> Result<SyscallResult, Errno> {
    if path.is_null() {
        return Err(Errno::EFAULT);
    }

    let path = unsafe { CStr::from_ptr(path.cast()) }.to_bytes();
    if path.is_empty() {
        return Err(Errno::ENOENT);
    }

    current.process().chdir(path)?;
    Ok(SyscallResult::Done(0))
}
