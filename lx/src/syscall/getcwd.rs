use core::slice;

use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::size_t;

pub fn sys_getcwd(current: &LxThread, buf: *mut u8, size: size_t) -> Result<SyscallResult, Errno> {
    if buf.is_null() {
        return Err(Errno::EFAULT);
    }

    let bytes = unsafe { slice::from_raw_parts_mut(buf, size) };
    let cwd = current.process().cwd();
    let len = cwd.absolute_path(bytes)?;
    Ok(SyscallResult::Done(len as c_long))
}
