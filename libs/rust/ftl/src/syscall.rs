use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::net::NetRxInfo;
use ftl_types::poll::Event;
use ftl_types::syscall::Syscall;
use ftl_types::thread::ExitReason;
use ftl_types::thread::Regs;
use ftl_types::thread::RegsKind;
use ftl_types::vmspace::PageAttrs;

use crate::arch::syscall1;
use crate::arch::syscall2;
use crate::arch::syscall3;
use crate::arch::syscall4;
use crate::arch::syscall6;

pub fn print(bytes: &[u8]) {
    let _ = syscall2(Syscall::Print, bytes.as_ptr() as usize, bytes.len());
}

pub fn thread_exit(reason: ExitReason) -> ! {
    let _ = syscall1(Syscall::ThreadExit, reason as usize);
    crate::arch::unreachable();
}

pub fn thread_create(
    isolate: HandleId,
    vmspace: HandleId,
    pc: usize,
    sp: usize,
    fault_pc: usize,
    cookie: usize,
) -> Result<HandleId, ErrorCode> {
    let ret = syscall6(
        Syscall::ThreadCreate,
        isolate.as_usize(),
        vmspace.as_usize(),
        pc,
        sp,
        fault_pc,
        cookie,
    )?;
    Ok(HandleId::new(ret))
}

pub fn thread_start(thread: HandleId) -> Result<(), ErrorCode> {
    syscall1(Syscall::ThreadStart, thread.as_usize())?;
    Ok(())
}

pub fn thread_write_regs(thread: HandleId, kind: RegsKind, regs: Regs) -> Result<(), ErrorCode> {
    syscall3(
        Syscall::ThreadWriteRegs,
        thread.as_usize(),
        kind as usize,
        &raw const regs as usize,
    )?;
    Ok(())
}

pub fn thread_copy_regs(src: HandleId, dest: HandleId, kind: RegsKind) -> Result<(), ErrorCode> {
    syscall3(
        Syscall::ThreadCopyRegs,
        src.as_usize(),
        dest.as_usize(),
        kind as usize,
    )?;
    Ok(())
}

pub fn poll_create() -> Result<HandleId, ErrorCode> {
    let ret = syscall1(Syscall::PollCreate, 0)?;
    Ok(HandleId::new(ret))
}

pub fn poll_wait(poll: HandleId) -> Result<Event, ErrorCode> {
    let ret = syscall1(Syscall::PollWait, poll.as_usize())?;
    Ok(Event::from_raw(ret as u32))
}

pub fn poll_notify(poll: HandleId) -> Result<(), ErrorCode> {
    syscall1(Syscall::PollNotify, poll.as_usize())?;
    Ok(())
}

pub fn net_acquire(
    poll: HandleId,
    kind: usize,
    our_ip: u32,
    our_port: u16,
) -> Result<HandleId, ErrorCode> {
    let ret = syscall4(
        Syscall::NetAcquire,
        poll.as_usize(),
        kind,
        our_ip as usize,
        our_port as usize,
    )?;
    Ok(HandleId::new(ret))
}

pub fn net_peek(net: HandleId, info: &mut NetRxInfo) -> Result<usize, ErrorCode> {
    syscall2(
        Syscall::NetPeek,
        net.as_usize(),
        info as *mut NetRxInfo as usize,
    )
}

pub fn net_recv(net: HandleId, token: usize, payload: &mut [u8]) -> Result<(), ErrorCode> {
    syscall4(
        Syscall::NetRecv,
        net.as_usize(),
        token,
        payload.as_mut_ptr() as usize,
        payload.len(),
    )?;
    Ok(())
}

pub fn net_send(net: HandleId, header: &[u8], payload: &[u8]) -> Result<(), ErrorCode> {
    syscall6(
        Syscall::NetSend,
        net.as_usize(),
        0,
        header.as_ptr() as usize,
        header.len(),
        payload.as_ptr() as usize,
        payload.len(),
    )?;
    Ok(())
}

pub fn vmspace_clone(source: HandleId) -> Result<HandleId, ErrorCode> {
    let ret = syscall1(Syscall::VmSpaceClone, source.as_usize())?;
    Ok(HandleId::new(ret))
}

pub fn vmspace_map(
    vmspace: HandleId,
    vmo: HandleId,
    uaddr: usize,
    attrs: PageAttrs,
) -> Result<(), ErrorCode> {
    syscall4(
        Syscall::VmSpaceMap,
        vmspace.as_usize(),
        vmo.as_usize(),
        uaddr,
        attrs.as_raw(),
    )?;
    Ok(())
}

pub fn vmo_create(len: usize) -> Result<HandleId, ErrorCode> {
    let ret = syscall1(Syscall::VmoCreate, len)?;
    Ok(HandleId::new(ret))
}

pub fn vmo_write(vmo: HandleId, offset: usize, buf: &[u8]) -> Result<(), ErrorCode> {
    syscall4(
        Syscall::VmoWrite,
        vmo.as_usize(),
        offset,
        buf.as_ptr() as usize,
        buf.len(),
    )?;
    Ok(())
}
