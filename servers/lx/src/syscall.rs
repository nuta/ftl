use ftl_api::info;
use ftl_api::thread::ContextData;
use ftl_api::thread::ContextKind;
use ftl_api::thread::FsBase;
use ftl_api::thread::SyscallArgs;
use ftl_api::thread::Thread;
use ftl_api::warn;

use crate::errno::Errno;
use crate::process::Process;

const SYS_WRITE: u64 = 1;
const SYS_WRITEV: u64 = 20;
const SYS_ARCH_PRCTL: u64 = 158;
const SYS_EXIT: u64 = 60;
const SYS_SET_TID_ADDRESS: u64 = 218;
const SYS_EXIT_GROUP: u64 = 231;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Iovec {
    pub iov_base: usize,
    pub iov_len: usize,
}

const ARCH_SET_FS: isize = 0x1002;

pub enum SyscallOutput {
    Done(Result<isize, Errno>),
    /// The thread exited.
    Exit,
}

pub fn handle_syscall(process: &Process, thread: &Thread, args: SyscallArgs) -> SyscallOutput {
    let retval = match args.n {
        SYS_EXIT | SYS_EXIT_GROUP => {
            info!("exited with code {}", args.arg0 as i64 as i32);
            thread.terminate().expect("terminate failed");
            return SyscallOutput::Exit;
        }
        SYS_WRITE => sys_write(process, args.arg0, args.arg1 as isize, args.arg2 as isize),
        SYS_WRITEV => sys_writev(process, args.arg0, args.arg1 as isize, args.arg2 as isize),
        SYS_ARCH_PRCTL => sys_arch_prctl(thread, args.arg0 as isize, args.arg1 as isize),
        SYS_SET_TID_ADDRESS => Ok(1000), // TODO:
        nr => {
            warn!("unimplemented syscall: nr={nr:x}");
            Err(Errno::ENOSYS)
        }
    };

    SyscallOutput::Done(retval)
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

// FIXME: Use correct types for args.
fn sys_writev(process: &Process, fd: u64, iov_uaddr: isize, iovcnt: isize) -> Result<isize, Errno> {
    if fd != 1 && fd != 2 {
        return Err(Errno::EBADF);
    }

    if iovcnt > 16 {
        return Err(Errno::EINVAL);
    }

    // FIXME: Terrible hack
    let mut iovecs_buf = [Iovec {
        iov_base: 0,
        iov_len: 0,
    }; 16];
    let iovecs_buf_bytes = unsafe {
        core::slice::from_raw_parts_mut(iovecs_buf.as_mut_ptr() as *mut u8, iovcnt as usize * 16)
    };

    // Read the iovecs from the user space.
    if process
        .vmspace()
        .read_bytes(iov_uaddr as usize, iovecs_buf_bytes)
        .is_err()
    {
        return Err(Errno::EFAULT);
    }

    let iovecs = &iovecs_buf[..iovcnt as usize];
    let mut total = 0isize;
    for iovec in iovecs {
        total += sys_write(process, fd, iovec.iov_base as isize, iovec.iov_len as isize)?;
    }

    Ok(total)
}

fn sys_arch_prctl(thread: &Thread, code: isize, addr: isize) -> Result<isize, Errno> {
    match code {
        ARCH_SET_FS => {
            let regs = ContextData {
                fsbase: FsBase { base: addr as u64 },
            };
            thread
                .set_context(ContextKind::Fsbase, &regs)
                .map_err(|_| Errno::EINVAL)?;
            Ok(0)
        }
        _ => Err(Errno::ENOSYS),
    }
}
