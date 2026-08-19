use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::Syscall;
use ftl_types::thread::ExitReason;
use ftl_types::vmspace::PageAttrs;

use crate::arch::syscall1;
use crate::arch::syscall2;
use crate::arch::syscall4;

pub fn print(bytes: &[u8]) {
    let _ = syscall2(Syscall::Print, bytes.as_ptr() as usize, bytes.len());
}

pub fn thread_exit(reason: ExitReason) -> ! {
    let _ = syscall1(Syscall::ThreadExit, reason as usize);
    crate::arch::unreachable();
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
