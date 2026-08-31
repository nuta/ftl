use core::slice;

use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_void;
use crate::types::errno::Errno;
use crate::types::size_t;

pub fn sys_read(
    current: &LxThread,
    fd: c_int,
    buf: *mut c_void,
    count: size_t,
) -> Result<c_long, Errno> {
    let file = {
        let process = current.process();
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    if count == 0 {
        return Ok(0);
    }

    let bytes = unsafe { slice::from_raw_parts_mut(buf.cast::<u8>(), count) };
    Ok(file.read(bytes, 0)? as c_long)
}
