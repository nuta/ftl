use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;

fn encode_wait_status(exit_status: c_int) -> c_int {
    (exit_status & 0xff) << 8
}

pub fn sys_wait4(
    current: &Thread,
    pid: c_int,
    wstatus: *mut c_int,
    options: c_int,
) -> Result<c_long, Errno> {
    if options != 0 {
        return Err(Errno::EINVAL);
    }

    let (pid, exit_status) = current.process().wait(pid)?;
    if !wstatus.is_null() {
        unsafe { wstatus.write(encode_wait_status(exit_status)) };
    }

    Ok(pid.as_int() as c_long)
}
