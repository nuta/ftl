mod arch_prctl;
mod exit_group;
mod set_tid_address;
mod write;
mod writev;

use ftl::info;

use self::arch_prctl::sys_arch_prctl;
use self::exit_group::sys_exit_group;
use self::set_tid_address::sys_set_tid_address;
use self::write::sys_write;
use self::writev::sys_writev;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_void;
use crate::types::errno::Errno;
use crate::types::sys::syscall::SYS_ARCH_PRCTL;
use crate::types::sys::syscall::SYS_EXIT_GROUP;
use crate::types::sys::syscall::SYS_SET_TID_ADDRESS;
use crate::types::sys::syscall::SYS_WRITE;
use crate::types::sys::syscall::SYS_WRITEV;
use crate::types::sys::uio::iovec;

pub extern "C" fn handle_syscall(
    arg0: usize,
    arg1: usize,
    arg2: usize,
    _arg3: usize,
    _arg4: usize,
    _arg5: usize,
    n: usize,
    cookie: usize,
) -> c_long {
    // SAFETY: The kernel returns the cookie we gave.
    let current = unsafe { Thread::from_cookie(cookie) };
    info!(
        "syscall: tid={}, n={}, [{:#x}, {:#x}, {:#x}]",
        current.tid(),
        n,
        arg0,
        arg1,
        arg2
    );

    let result = match n {
        SYS_WRITE => sys_write(&current, arg0 as c_int, arg1 as *const c_void, arg2),
        SYS_WRITEV => sys_writev(&current, arg0 as c_int, arg1 as *const iovec, arg2 as c_int),
        SYS_ARCH_PRCTL => sys_arch_prctl(&current, arg0 as c_int, arg1),
        SYS_SET_TID_ADDRESS => sys_set_tid_address(&current, arg0 as *mut c_int),
        SYS_EXIT_GROUP => sys_exit_group(&current, arg0 as c_int),
        _ => Err(Errno::ENOSYS),
    };

    match result {
        Ok(retval) => retval,
        Err(errno) => -(errno.as_int() as c_long),
    }
}
