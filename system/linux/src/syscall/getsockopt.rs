use core::cmp::min;
use core::mem::size_of;

use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;

pub fn sys_getsockopt(
    current: &Thread,
    fd: c_int,
    level: c_int,
    optname: c_int,
    optval: *mut u8,
    optlen: *mut u32,
) -> Result<SyscallResult, Errno> {
    if optval.is_null() || optlen.is_null() {
        return Err(Errno::EFAULT);
    }

    let file = {
        let process = current.process();
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    let value = file.getsockopt(level, optname)?;
    let value_bytes = value.to_ne_bytes();

    let buf_len = unsafe { optlen.read_unaligned() } as usize;
    let copy_len = min(buf_len, size_of::<c_int>());
    unsafe {
        optval.copy_from_nonoverlapping(value_bytes.as_ptr(), copy_len);
        optlen.write_unaligned(copy_len as u32);
    }

    Ok(SyscallResult::Done(0))
}
