use core::ffi::CStr;

use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_ulong;
use crate::types::errno::Errno;
use crate::types::sys::prctl::PR_GET_NAME;
use crate::types::sys::prctl::PR_SET_NAME;
use crate::types::sys::sched::TASK_COMM_LEN;

pub fn sys_prctl(current: &LxThread, option: c_int, arg2: c_ulong) -> Result<SyscallResult, Errno> {
    match option {
        PR_SET_NAME => {
            let name = arg2 as *const u8;
            if name.is_null() {
                return Err(Errno::EFAULT);
            }

            let name = unsafe { CStr::from_ptr(name.cast()) }.to_bytes();
            current.set_name(name);
            Ok(SyscallResult::Done(0))
        }
        PR_GET_NAME => {
            let buf = arg2 as *mut [u8; TASK_COMM_LEN];
            if buf.is_null() {
                return Err(Errno::EFAULT);
            }

            let name = current.name();
            unsafe { buf.write(name) };
            Ok(SyscallResult::Done(0))
        }
        _ => Err(Errno::EINVAL),
    }
}
