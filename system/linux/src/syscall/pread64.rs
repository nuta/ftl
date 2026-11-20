use core::slice;

use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_void;
use crate::types::errno::Errno;
use crate::types::off_t;
use crate::types::size_t;
use crate::wait_queue::Sleep;

pub fn sys_pread64(
    current: &Thread,
    fd: c_int,
    buf: *mut c_void,
    count: size_t,
    offset: off_t,
) -> Result<SyscallResult, Errno> {
    let process = current.process();
    let file = {
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    let Ok(offset) = usize::try_from(offset) else {
        return Err(Errno::EINVAL);
    };

    if count == 0 {
        return Ok(SyscallResult::Done(0));
    }

    if buf.is_null() {
        return Err(Errno::EFAULT);
    }

    let bytes = unsafe { slice::from_raw_parts_mut(buf.cast::<u8>(), count) };
    let read_len = file.pread(
        bytes,
        offset,
        Sleep::Interruptible {
            thread: current,
            process: &process,
        },
    )?;
    Ok(SyscallResult::Done(read_len as c_long))
}
