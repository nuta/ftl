use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::wait::WNOHANG;

const SUPPORTED_FLAGS: c_int = WNOHANG;

fn encode_wait_status(exit_status: c_int) -> c_int {
    (exit_status & 0xff) << 8
}

pub fn sys_wait4(
    current: &LxThread,
    pid: c_int,
    wstatus: *mut c_int,
    options: c_int,
) -> Result<SyscallResult, Errno> {
    if options & !SUPPORTED_FLAGS != 0 {
        return Err(Errno::EINVAL);
    }

    let wnohang = options & WNOHANG != 0;
    let retval = match current.process().wait(pid, wnohang) {
        Ok(Some((pid, exit_status))) => {
            if !wstatus.is_null() {
                unsafe { wstatus.write(encode_wait_status(exit_status)) };
            }

            pid.as_int() as c_long
        }
        Ok(None) => 0,
        Err(errno) => {
            return Err(errno);
        }
    };

    Ok(SyscallResult::Done(retval as c_long))
}
