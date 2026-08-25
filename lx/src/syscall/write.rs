use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_void;
use crate::types::errno::Errno;
use crate::types::size_t;

pub fn sys_write(
    _current: &Thread,
    fd: c_int,
    buf: *const c_void,
    count: size_t,
) -> Result<c_long, Errno> {
    // TODO: fd table
    let _ = fd;

    let bytes = unsafe { core::slice::from_raw_parts(buf.cast::<u8>(), count) };
    ftl::syscall::print(bytes);
    Ok(count as c_long)
}
