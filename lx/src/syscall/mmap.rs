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
    fd: c_int,
    offset: i64,
) -> Result<SyscallResult, Errno> {
    let fixed = flags & MAP_FIXED != 0;
    let addr = if flags & MAP_ANONYMOUS != 0 {
        current.vm().mmap_anonymous(addr, len, prot, fixed)?
    } else {
        let file = {
            let process = current.process();
            let fd_table = process.fd_table().lock();
            fd_table.get(fd)?.file().clone()
        };

        let Ok(offset) = usize::try_from(offset) else {
            return Err(Errno::EINVAL);
        };

        current
            .vm()
            .mmap_file(addr, len, prot, fixed, file, offset)?
    };

    Ok(SyscallResult::Done(addr as c_long))
}
