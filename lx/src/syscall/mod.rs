use ftl::info;

use crate::thread::ThreadCtx;

const SYS_WRITE: usize = 1;
const ENOSYS: isize = 38;

pub extern "C" fn handle_syscall(
    arg0: usize,
    arg1: usize,
    arg2: usize,
    _arg3: usize,
    _arg4: usize,
    _arg5: usize,
    n: usize,
    cookie: usize,
) -> isize {
    // SAFETY: The kernel returns the cookie provided to `Thread::new`.
    let thread = unsafe { ThreadCtx::from_cookie(cookie) };
    info!(
        "syscall: tid={}, n={}, [{:#x}, {:#x}, {:#x}]",
        thread.tid(),
        n,
        arg0,
        arg1,
        arg2
    );
    if n == SYS_WRITE {
        let bytes = unsafe { core::slice::from_raw_parts(arg1 as *const u8, arg2) };
        ftl::syscall::print(bytes);
        arg2 as isize
    } else {
        -ENOSYS
    }
}
