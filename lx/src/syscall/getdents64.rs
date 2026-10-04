use core::slice;

use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_void;
use crate::types::errno::Errno;
use crate::types::size_t;

pub fn sys_getdents64(
    current: &LxThread,
    fd: c_int,
    dirp: *mut c_void,
    count: size_t,
) -> Result<SyscallResult, Errno> {
    let file = {
        let process = current.process();
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    if dirp.is_null() {
        return Err(Errno::EFAULT);
    }

    let bytes = unsafe { slice::from_raw_parts_mut(dirp.cast(), count) };
    let n = file.getdents(bytes)?;
    Ok(SyscallResult::Done(n as c_long))
}
