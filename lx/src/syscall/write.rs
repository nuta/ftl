use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_void;
use crate::types::errno::Errno;
use crate::types::size_t;

pub fn sys_write(
    current: &Thread,
    fd: c_int,
    buf: *const c_void,
    count: size_t,
) -> Result<c_long, Errno> {
    let file = {
        let process = current.process();
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    let bytes = unsafe { core::slice::from_raw_parts(buf.cast::<u8>(), count) };
    let n = file.write(bytes, 0)?;
    Ok(n.try_into().unwrap()) // FIXME: Handle overflow
}
