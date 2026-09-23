use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_void;
use crate::types::errno::Errno;
use crate::types::size_t;
use crate::wait_queue::Sleep;

pub fn sys_write(
    current: &LxThread,
    fd: c_int,
    buf: *const c_void,
    count: size_t,
) -> Result<c_long, Errno> {
    let process = current.process();
    let file = {
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    if count == 0 {
        return Ok(0);
    }

    if buf.is_null() {
        return Err(Errno::EFAULT);
    }

    let bytes = unsafe { core::slice::from_raw_parts(buf.cast::<u8>(), count) };
    let n = file.write(bytes, Sleep::Interruptible(&process))?;
    Ok(n.try_into().unwrap()) // FIXME: Handle overflow
}
