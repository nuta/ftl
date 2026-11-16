use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::mman::MAP_ANONYMOUS;
use crate::types::sys::mman::MAP_FIXED;

pub fn sys_mmap(
    current: &LxThread,
    addr: usize,
    len: usize,
    prot: c_int,
    flags: c_int,
    _fd: c_int,
    _offset: i64,
) -> Result<SyscallResult, Errno> {
    let addr = if flags & MAP_ANONYMOUS != 0 {
        let fixed = flags & MAP_FIXED != 0;
        current.vm().mmap_anonymous(addr, len, prot, fixed)?
    } else {
        // TODO: File-backed mappings are not supported yet.
        return Err(Errno::ENOSYS);
    };

    Ok(SyscallResult::Done(addr as c_long))
}
