use core::slice;

use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_long;
use crate::types::c_unsigned;
use crate::types::c_void;
use crate::types::errno::Errno;
use crate::types::size_t;

pub fn sys_getrandom(
    _current: &LxThread,
    buf: *mut c_void,
    size: size_t,
    _flags: c_unsigned,
) -> Result<SyscallResult, Errno> {
    if size == 0 {
        return Ok(SyscallResult::Done(0));
    }

    if buf.is_null() {
        return Err(Errno::EFAULT);
    }

    // TODO: support flags.
    let bytes = unsafe { slice::from_raw_parts_mut(buf.cast::<u8>(), size) };
    ftl::random::read(bytes)?;
    Ok(SyscallResult::Done(size as c_long))
}
