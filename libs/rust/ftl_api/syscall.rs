//! Low-level system call interfaces.
use ftl_types::address::PAddr;
use ftl_types::address::VAddr;
use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;
use ftl_types::interrupt::Irq;
use ftl_types::message::MessageBuffer;
use ftl_types::message::MessageInfo;
use ftl_types::poll::PollEvent;
use ftl_types::poll::PollSyscallResult;
use ftl_types::signal::SignalBits;
use ftl_types::syscall::SyscallNumber;
use ftl_types::syscall::VsyscallEntry;
use ftl_types::syscall::VsyscallPage;
use ftl_types::vmspace::PageProtect;

#[cfg(target_arch = "riscv64")]
#[inline(never)] // TODO: List clobbered registers explicitly in asm!
pub fn syscall(
    n: SyscallNumber,
    mut a0: isize,
    a1: isize,
    a2: isize,
    a3: isize,
    a4: isize,
) -> Result<isize, FtlError> {
    use core::arch::asm;

    unsafe {
        asm!(
            "jalr a6",
            inout("a0") a0,
            in("a1") a1,
            in("a2") a2,
            in("a3") a3,
            in("a4") a4,
            in("a5") n as isize,
            in("a6") VSYSCALL_ENTRY
        );
    }

    if a0 < 0 {
        unsafe { Err(core::mem::transmute::<isize, FtlError>(a0)) }
    } else {
        Ok(a0)
    }
}

#[cfg(target_arch = "x86_64")]
#[inline(never)] // TODO: List clobbered registers explicitly in asm!
pub fn syscall(
    n: SyscallNumber,
    rdi: isize,
    rsi: isize,
    rdx: isize,
    rcx: isize,
    r8: isize,
) -> Result<isize, FtlError> {
    use core::arch::asm;

    let mut rax: isize = unsafe { VSYSCALL_ENTRY as isize };
    unsafe {
        asm!(
            "call rax",
            in("rdi") rdi,
            in("rsi") rsi,
            in("rdx") rdx,
            in("rcx") rcx,
            in("r8") r8,
            in("r9") n as isize,
            inout("rax") rax
        );
    }

    if rax < 0 {
        unsafe { Err(core::mem::transmute::<isize, FtlError>(rax)) }
    } else {
        Ok(rax)
    }
}

#[cfg(not(target_os = "none"))]
pub fn syscall(
    _n: SyscallNumber,
    _a0: isize,
    _a1: isize,
    _a2: isize,
    _a3: isize,
    _a4: isize,
) -> Result<isize, FtlError> {
    panic!("syscall not implemented in test mode");
}

/// Invokes a system call with no arguments.
pub fn syscall0(n: SyscallNumber) -> Result<isize, FtlError> {
    syscall(n, 0, 0, 0, 0, 0)
}

/// Invokes a system call with 1 argument.
pub fn syscall1(n: SyscallNumber, a0: isize) -> Result<isize, FtlError> {
    syscall(n, a0, 0, 0, 0, 0)
}

/// Invokes a system call with 2 arguments.
pub fn syscall2(n: SyscallNumber, a0: isize, a1: isize) -> Result<isize, FtlError> {
    syscall(n, a0, a1, 0, 0, 0)
}

/// Invokes a system call with 3 arguments.
pub fn syscall3(n: SyscallNumber, a0: isize, a1: isize, a2: isize) -> Result<isize, FtlError> {
    syscall(n, a0, a1, a2, 0, 0)
}

/// Invokes a system call with 4 arguments.
pub fn syscall4(
    n: SyscallNumber,
    a0: isize,
    a1: isize,
    a2: isize,
    a3: isize,
) -> Result<isize, FtlError> {
    syscall(n, a0, a1, a2, a3, 0)
}

/// Invokes a system call with 5 arguments.
pub fn syscall5(
    n: SyscallNumber,
    a0: isize,
    a1: isize,
    a2: isize,
    a3: isize,
    a4: isize,
) -> Result<isize, FtlError> {
    syscall(n, a0, a1, a2, a3, a4)
}

/// Closes a handle.
pub fn handle_close(handle: HandleId) -> Result<(), FtlError> {
    syscall1(SyscallNumber::HandleClose, handle.as_isize())?;
    Ok(())
}

/// Prints a string to the debug console.
pub fn console_write(s: &[u8]) -> Result<(), FtlError> {
    syscall2(
        SyscallNumber::ConsoleWrite,
        s.as_ptr() as isize,
        s.len() as isize,
    )?;
    Ok(())
}

/// Creates a folio at an arbitrary physical address.
pub fn folio_create(len: usize) -> Result<HandleId, FtlError> {
    let ret = syscall1(SyscallNumber::FolioCreate, len as isize)?;
    let handle_id = HandleId::from_raw_isize_truncated(ret);
    Ok(handle_id)
}

/// Creates a folio at a fixed physical address.
pub fn folio_create_fixed(paddr: PAddr, len: usize) -> Result<HandleId, FtlError> {
    let ret = syscall2(
        SyscallNumber::FolioCreateFixed,
        paddr.as_usize() as isize,
        len as isize,
    )?;
    let handle_id = HandleId::from_raw_isize_truncated(ret);
    Ok(handle_id)
}

/// Returns the physical address of a folio.
pub fn folio_paddr(handle: HandleId) -> Result<usize, FtlError> {
    let ret = syscall1(SyscallNumber::FolioPAddr, handle.as_isize())?;
    Ok(ret as usize)
}

/// Maps a folio into a virtual address space.
pub fn vmspace_map(
    handle: HandleId,
    len: usize,
    folio: HandleId,
    prot: PageProtect,
) -> Result<VAddr, FtlError> {
    let ret = syscall4(
        SyscallNumber::VmSpaceMap,
        handle.as_isize(),
        len as isize,
        folio.as_isize(),
        prot.as_raw() as isize,
    )?;

    Ok(VAddr::new(ret as usize))
}

/// Creates a poll object.
pub fn poll_create() -> Result<HandleId, FtlError> {
    let ret = syscall0(SyscallNumber::PollCreate)?;
    let handle_id = HandleId::from_raw_isize_truncated(ret);
    Ok(handle_id)
}

/// Adds a target handle to a poll object.
pub fn poll_add(poll: HandleId, object: HandleId, interests: PollEvent) -> Result<(), FtlError> {
    syscall3(
        SyscallNumber::PollAdd,
        poll.as_isize(),
        object.as_isize(),
        interests.as_raw() as isize,
    )?;
    Ok(())
}

/// Removes a target handle from a poll object.
pub fn poll_remove(poll: HandleId, object: HandleId) -> Result<(), FtlError> {
    syscall2(
        SyscallNumber::PollRemove,
        poll.as_isize(),
        object.as_isize(),
    )?;
    Ok(())
}

/// Waits for an event on a poll object. Blocking.
pub fn poll_wait(poll: HandleId) -> Result<PollSyscallResult, FtlError> {
    let ret = syscall1(SyscallNumber::PollWait, poll.as_isize())?;
    Ok(PollSyscallResult::from_raw(ret))
}

/// Creates a channel pair.
pub fn channel_create() -> Result<(HandleId, HandleId), FtlError> {
    let ret = syscall0(SyscallNumber::ChannelCreate)?;
    let handle0 = HandleId::from_raw_isize_truncated(ret);
    let handle1 = HandleId::from_raw_isize_truncated(ret + 1);
    Ok((handle0, handle1))
}

/// Sends a message to a channel.
pub fn channel_send(
    handle: HandleId,
    msginfo: MessageInfo,
    msgbuffer: *const MessageBuffer,
) -> Result<(), FtlError> {
    syscall3(
        SyscallNumber::ChannelSend,
        handle.as_isize(),
        msginfo.as_raw(),
        msgbuffer as isize,
    )?;
    Ok(())
}

/// Receives a message from a channel. Blocking.
pub fn channel_recv(
    handle: HandleId,
    msgbuffer: *mut MessageBuffer,
) -> Result<MessageInfo, FtlError> {
    let ret = syscall2(
        SyscallNumber::ChannelRecv,
        handle.as_isize(),
        msgbuffer as isize,
    )?;
    Ok(MessageInfo::from_raw(ret))
}

/// Tries to receive a message from a channel. Non-blocking.
pub fn channel_try_recv(
    handle: HandleId,
    msgbuffer: *mut MessageBuffer,
) -> Result<MessageInfo, FtlError> {
    let ret = syscall2(
        SyscallNumber::ChannelTryRecv,
        handle.as_isize(),
        msgbuffer as isize,
    )?;
    Ok(MessageInfo::from_raw(ret))
}

/// Sends a message and then receives a reply. Blocking.
pub fn channel_call(
    handle: HandleId,
    msginfo: MessageInfo,
    msgbuffer: *const MessageBuffer,
) -> Result<MessageInfo, FtlError> {
    let ret = syscall3(
        SyscallNumber::ChannelCall,
        handle.as_isize(),
        msginfo.as_raw(),
        msgbuffer as isize,
    )?;
    Ok(MessageInfo::from_raw(ret))
}

/// Creates a signal object.
pub fn signal_create() -> Result<HandleId, FtlError> {
    let ret = syscall0(SyscallNumber::SignalCreate)?;
    let handle_id = HandleId::from_raw_isize_truncated(ret);
    Ok(handle_id)
}

/// Updates a signal's value.
pub fn signal_update(handle: HandleId, value: SignalBits) -> Result<(), FtlError> {
    syscall2(
        SyscallNumber::SignalUpdate,
        handle.as_isize(),
        value.as_i32() as isize,
    )?;
    Ok(())
}

/// Reads and clears a signal's value atomically. Non-blocking.
pub fn signal_clear(handle: HandleId) -> Result<SignalBits, FtlError> {
    let ret = syscall1(SyscallNumber::SignalClear, handle.as_isize())?;
    Ok(SignalBits::from_raw(ret as i32))
}

/// Creates a interrupt object.
pub fn interrupt_create(irq: Irq) -> Result<HandleId, FtlError> {
    let ret = syscall1(SyscallNumber::InterruptCreate, irq.as_usize() as isize)?;
    let handle_id = HandleId::from_raw_isize_truncated(ret);
    Ok(handle_id)
}

/// Acknowledges an interrupt.
pub fn interrupt_ack(handle: HandleId) -> Result<(), FtlError> {
    syscall1(SyscallNumber::InterruptAck, handle.as_isize())?;
    Ok(())
}

/// Exits the current process.
pub fn process_exit() -> ! {
    syscall0(SyscallNumber::ProcessExit).unwrap();
    unreachable!();
}

static mut VSYSCALL_ENTRY: *const VsyscallEntry = core::ptr::null();

pub(crate) fn set_vsyscall(vsyscall: &VsyscallPage) {
    unsafe {
        VSYSCALL_ENTRY = vsyscall.entry;
    }
}
