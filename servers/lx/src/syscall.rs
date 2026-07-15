use ftl_api::info;
use ftl_api::thread::SyscallArgs;
use ftl_api::thread::Thread;

use crate::errno::Errno;
use crate::process::Process;

const SYS_WRITE: u64 = 1;

pub fn handle_syscall(
    process: &Process,
    _thread: &Thread,
    args: SyscallArgs,
) -> Result<isize, Errno> {
    match args.n {
        SYS_WRITE => sys_write(process, args.arg0, args.arg1 as isize, args.arg2 as isize),
        nr => {
            info!("unimplemented syscall: nr={nr:x}");
            Err(Errno::ENOSYS)
        }
    }
}

fn sys_write(process: &Process, fd: u64, uaddr: isize, count: isize) -> Result<isize, Errno> {
    if fd != 1 && fd != 2 {
        // stdout/stderr only for now
        return Err(Errno::EBADF);
    }

    // FIXME: How should we validate uaddr/count?

    let vmspace = process.vmspace();
    let mut remaining = count as usize;
    let mut uaddr = uaddr as usize;
    let mut chunk = [0u8; 256];
    while remaining > 0 {
        let n = core::cmp::min(remaining, chunk.len());
        if vmspace.read_bytes(uaddr, &mut chunk[..n]).is_err() {
            return Err(Errno::EFAULT);
        }

        let text = core::str::from_utf8(&chunk[..n]).unwrap_or("invalid utf8");
        info!("write: {}", text.trim_ascii_end());

        uaddr += n;
        remaining -= n;
    }

    Ok(count)
}
