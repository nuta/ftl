use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;
use ftl_types::message::MessageInfo;
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

fn retval_to_handle(retval: isize) -> Result<HandleId, FtlError> {
    let id: i32 = (retval & HANDLE_ID_MASK)
        .try_into()
        .map_err(|_| FtlError::InvalidSyscallReturnValue)?;

    Ok(HandleId::from_raw(id))
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
    let handle0 = retval_to_handle(ret)?;
    let handle1 = retval_to_handle(ret >> HANDLE_ID_BITS)?;
    Ok((handle0, handle1))
}

pub fn channel_send(
    handle: HandleId,
    msginfo: MessageInfo,
    buf: *const u8,
) -> Result<(), FtlError> {
    syscall3(
        SyscallNumber::ChannelSend,
        handle.as_isize(),
        msginfo.as_raw(),
        buf as isize,
    )?;
    Ok(())
}

pub fn channel_recv(handle: HandleId, buf: *mut u8) -> Result<MessageInfo, FtlError> {
    let ret = syscall2(SyscallNumber::ChannelRecv, handle.as_isize(), buf as isize)?;
    Ok(MessageInfo::from_raw(ret))
}

pub(crate) fn set_vsyscall(vsyscall: &'static VsyscallPage) {
    *VSYSCALL_PAGE.lock() = Some(vsyscall);
}
