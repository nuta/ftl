use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::mman::MAP_ANONYMOUS;

pub fn sys_mmap(
    current: &LxThread,
    addr: usize,
    len: usize,
    prot: c_int,
    flags: c_int,
    _fd: c_int,
    _offset: i64,
) -> Result<c_long, Errno> {
    let addr = if flags & MAP_ANONYMOUS != 0 {
        current.vm().mmap_anonymous(addr, len, prot)?
    } else {
        // TODO: MAP_FIXED is not supported yet.
        return Err(Errno::ENOSYS);
    };

    Ok(addr as c_long)
}
