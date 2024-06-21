use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;
use ftl_types::syscall::SyscallNumber;
use ftl_types::syscall::VsyscallPage;
use spin::Mutex;

const HANDLE_ID_BITS: usize = 20;
const HANDLE_ID_MASK: isize = (1 << HANDLE_ID_BITS) - 1;

static VSYSCALL_PAGE: Mutex<Option<&'static VsyscallPage>> = Mutex::new(None);

pub fn syscall(
    n: SyscallNumber,
    a0: isize,
    a1: isize,
    a2: isize,
    a3: isize,
    a4: isize,
    a5: isize,
) -> Result<isize, FtlError> {
    let vsyscall = VSYSCALL_PAGE.lock().expect("vsyscall not set");
    (vsyscall.entry)(n as isize, a0, a1, a2, a3, a4, a5)
}

pub fn syscall0(n: SyscallNumber) -> Result<isize, FtlError> {
    syscall(n, 0, 0, 0, 0, 0, 0)
}

pub fn syscall1(n: SyscallNumber, a0: isize) -> Result<isize, FtlError> {
    syscall(n, a0, 0, 0, 0, 0, 0)
}

pub fn syscall2(n: SyscallNumber, a0: isize, a1: isize) -> Result<isize, FtlError> {
    syscall(n, a0, a1, 0, 0, 0, 0)
}

pub fn syscall3(n: SyscallNumber, a0: isize, a1: isize, a2: isize) -> Result<isize, FtlError> {
    syscall(n, a0, a1, a2, 0, 0, 0)
}

pub fn syscall4(
    n: SyscallNumber,
    a0: isize,
    a1: isize,
    a2: isize,
    a3: isize,
) -> Result<isize, FtlError> {
    syscall(n, a0, a1, a2, a3, 0, 0)
}

pub fn syscall5(
    n: SyscallNumber,
    a0: isize,
    a1: isize,
    a2: isize,
    a3: isize,
    a4: isize,
) -> Result<isize, FtlError> {
    syscall(n, a0, a1, a2, a3, a4, 0)
}

pub fn syscall6(
    n: SyscallNumber,
    a0: isize,
    a1: isize,
    a2: isize,
    a3: isize,
    a4: isize,
    a5: isize,
) -> Result<isize, FtlError> {
    syscall(n, a0, a1, a2, a3, a4, a5)
}

pub fn handle_close(handle: HandleId) -> Result<(), FtlError> {
    syscall1(SyscallNumber::HandleClose, handle.as_isize())?;
    Ok(())
}

pub fn print(s: &[u8]) -> Result<(), FtlError> {
    syscall2(SyscallNumber::Print, s.as_ptr() as isize, s.len() as isize)?;
    Ok(())
}

pub fn channel_create() -> Result<(HandleId, HandleId), FtlError> {
    let ret = syscall0(SyscallNumber::ChannelCreate)?;
    let handle0 =
        HandleId::from_raw(ret & HANDLE_ID_MASK).ok_or(FtlError::InvalidSyscallReturnValue)?;
    let handle1 =
        HandleId::from_raw(ret >> HANDLE_ID_BITS).ok_or(FtlError::InvalidSyscallReturnValue)?;
    Ok((handle0, handle1))
}

pub fn channel_send(
    handle: HandleId,
    header: usize,
    buf: &[u8],
    handles: &[HandleId],
) -> Result<(), FtlError> {
    syscall6(
        SyscallNumber::ChannelSend,
        handle.as_isize(),
        header as isize,
        buf.as_ptr() as isize,
        buf.len() as isize,
        handles.as_ptr() as isize,
        handles.len() as isize,
    )?;
    Ok(())
}

pub fn poll_create() -> Result<HandleId, FtlError> {
    let handle = syscall0(SyscallNumber::PollCreate)?;
    Ok(HandleId::from_raw(handle).ok_or(FtlError::InvalidSyscallReturnValue)?)
}

pub fn poll_add(poll: HandleId, handle: HandleId) -> Result<(), FtlError> {
    syscall2(SyscallNumber::PollAdd, poll.as_isize(), handle.as_isize())?;
    Ok(())
}

pub fn poll_wait(handle: HandleId) -> Result<isize, FtlError> {
    syscall1(SyscallNumber::PollWait, handle.as_isize())
}

pub(crate) fn set_vsyscall(vsyscall: &'static VsyscallPage) {
    *VSYSCALL_PAGE.lock() = Some(vsyscall);
}
