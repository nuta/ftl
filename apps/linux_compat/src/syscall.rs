use ftl::prelude::*;
use ftl::sync::Arc;

use crate::errno::Errno;
use crate::fs::Fd;
use crate::thread::LxThread;

const SYS_WRITE: usize = 1;

#[derive(Debug)]
pub enum SyscallResult {
    Return(usize),
    Error(Errno),
    Exit,
}

fn sys_write(
    thread: &Arc<LxThread>,
    a0: usize,
    a1: usize,
    a2: usize,
) -> Result<SyscallResult, Errno> {
    let fd = Fd::from_usize(a0);
    Ok(SyscallResult::Return(0))
}

pub fn handle_syscall(
    thread: &Arc<LxThread>,
    n: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
) {
    let result = match n {
        SYS_WRITE => sys_write(thread, a0, a1, a2),
        _ => {
            warn!("unknown syscall: {}", n);
            Err(Errno::ENOSYS)
        }
    };

    todo!("handle syscall result: {:?}", result);
}
