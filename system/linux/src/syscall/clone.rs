use ftl::trace;

use super::SyscallResult;
use crate::arch::SyscallFrame;
use crate::process::PId;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_ulong;
use crate::types::errno::Errno;
use crate::types::signal::SIGCHLD;
use crate::types::sys::sched::CLONE_CHILD_CLEARTID;
use crate::types::sys::sched::CLONE_CHILD_SETTID;
use crate::types::sys::sched::CLONE_DETACHED;
use crate::types::sys::sched::CLONE_FILES;
use crate::types::sys::sched::CLONE_FLAGS_MASK;
use crate::types::sys::sched::CLONE_FS;
use crate::types::sys::sched::CLONE_PARENT_SETTID;
use crate::types::sys::sched::CLONE_SETTLS;
use crate::types::sys::sched::CLONE_SIGHAND;
use crate::types::sys::sched::CLONE_SYSVSEM;
use crate::types::sys::sched::CLONE_THREAD;
use crate::types::sys::sched::CLONE_VM;

fn clone_thread(
    current: &Thread,
    frame: &SyscallFrame,
    flags: c_ulong,
    stack: usize,
    parent_tid: *mut c_int,
    child_tid: *mut c_int,
    tls: c_ulong,
) -> Result<PId, Errno> {
    // Threads in the system must share these resources (simply because unimplemented).
    const REQUIRED_FLAGS: c_ulong =
        CLONE_VM | CLONE_FS | CLONE_FILES | CLONE_SIGHAND | CLONE_THREAD;

    const SUPPORTED_FLAGS: c_ulong = REQUIRED_FLAGS
        | CLONE_SYSVSEM
        | CLONE_SETTLS
        | CLONE_PARENT_SETTID
        | CLONE_CHILD_CLEARTID
        | CLONE_DETACHED;

    let flags = flags & !CLONE_FLAGS_MASK;
    if flags & REQUIRED_FLAGS != REQUIRED_FLAGS {
        trace!("clone: required thread flags missing {:#x}", flags);
        return Err(Errno::EINVAL);
    }

    if flags & !SUPPORTED_FLAGS != 0 {
        trace!("clone: unsupported thread flags {:#x}", flags);
        return Err(Errno::EINVAL);
    }

    if stack == 0 {
        trace!("clone: stack is required for cloning threads");
        return Err(Errno::EINVAL);
    }

    let tls = if flags & CLONE_SETTLS != 0 {
        Some(tls)
    } else {
        None
    };

    let parent_tid = if flags & CLONE_PARENT_SETTID != 0 {
        Some(parent_tid)
    } else {
        None
    };

    let clear_child_tid = if flags & CLONE_CHILD_CLEARTID != 0 {
        Some(child_tid.addr())
    } else {
        None
    };

    current
        .process()
        .spawn_thread(current, frame, stack, tls, parent_tid, clear_child_tid)
}

fn clone_process(
    current: &Thread,
    frame: &SyscallFrame,
    flags: c_ulong,
    child_tid: *mut c_int,
) -> Result<PId, Errno> {
    const SUPPORTED_FLAGS: c_ulong = CLONE_CHILD_SETTID | CLONE_CHILD_CLEARTID;

    let exit_signal = (flags & CLONE_FLAGS_MASK) as c_int;
    if exit_signal != SIGCHLD {
        trace!("clone: unsupported exit signal {}", exit_signal);
        return Err(Errno::EINVAL);
    }

    let flags = flags & !CLONE_FLAGS_MASK;
    if flags & !SUPPORTED_FLAGS != 0 {
        trace!("clone: unsupported flags {:#x}", flags & !SUPPORTED_FLAGS);
        return Err(Errno::EINVAL);
    }

    let set_child_tid = if flags & CLONE_CHILD_SETTID != 0 {
        Some(child_tid.addr())
    } else {
        None
    };

    let clear_child_tid = if flags & CLONE_CHILD_CLEARTID != 0 {
        Some(child_tid.addr())
    } else {
        None
    };

    current
        .process()
        .fork(current, frame, set_child_tid, clear_child_tid)
}

pub fn sys_clone(
    current: &Thread,
    frame: &SyscallFrame,
    flags: c_ulong,
    stack: usize,
    parent_tid: *mut c_int,
    child_tid: *mut c_int,
    tls: c_ulong,
) -> Result<SyscallResult, Errno> {
    let pid = if flags & CLONE_THREAD != 0 {
        clone_thread(current, frame, flags, stack, parent_tid, child_tid, tls)?
    } else {
        if stack != 0 {
            // TODO: Support a new stack for the child.
            return Err(Errno::EINVAL);
        }

        clone_process(current, frame, flags, child_tid)?
    };

    Ok(SyscallResult::Done(pid.as_int() as c_long))
}
