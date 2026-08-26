mod arch_prctl;
mod execve;
mod exit_group;
mod fork;
mod set_tid_address;
mod write;
mod writev;

use ftl::info;

use self::arch_prctl::sys_arch_prctl;
use self::execve::sys_execve;
use self::exit_group::sys_exit_group;
use self::fork::sys_fork;
use self::set_tid_address::sys_set_tid_address;
use self::write::sys_write;
use self::writev::sys_writev;
use crate::arch::SyscallFrame;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_void;
use crate::types::errno::Errno;
use crate::types::sys::syscall::SYS_ARCH_PRCTL;
use crate::types::sys::syscall::SYS_EXECVE;
use crate::types::sys::syscall::SYS_EXIT_GROUP;
use crate::types::sys::syscall::SYS_FORK;
use crate::types::sys::syscall::SYS_SET_TID_ADDRESS;
use crate::types::sys::syscall::SYS_WRITE;
use crate::types::sys::syscall::SYS_WRITEV;
use crate::types::sys::uio::iovec;

pub extern "C" fn handle_syscall(frame: *const SyscallFrame) -> c_long {
    // SAFETY: `syscall_handler` passes its register frame.
    let frame = unsafe { &*frame };
    let nr = frame.nr;
    let arg0 = frame.arg0();
    let arg1 = frame.arg1();
    let arg2 = frame.arg2();

    // SAFETY: The kernel returns the cookie we gave.
    let current = unsafe { Thread::from_cookie(frame.cookie) };
    info!(
        "syscall: tid={}, n={}, [{:#x}, {:#x}, {:#x}]",
        current.tid(),
        nr,
        arg0,
        arg1,
        arg2
    );

    let result = match nr {
        SYS_WRITE => sys_write(&current, arg0 as c_int, arg1 as *const c_void, arg2),
        SYS_WRITEV => sys_writev(&current, arg0 as c_int, arg1 as *const iovec, arg2 as c_int),
        SYS_FORK => sys_fork(&current, frame as *const SyscallFrame as usize),
        SYS_EXECVE => {
            sys_execve(
                &current,
                arg0 as *const u8,
                arg1 as *const *const u8,
                arg2 as *const *const u8,
            )
        }
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
