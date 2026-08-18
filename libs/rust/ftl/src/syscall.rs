use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::Syscall;
use ftl_types::thread::ExitReason;

use crate::arch::syscall1;
use crate::arch::syscall2;

pub fn print(bytes: &[u8]) {
    let _ = syscall2(Syscall::Print, bytes.as_ptr() as usize, bytes.len());
}

pub fn thread_exit(reason: ExitReason) -> ! {
    let _ = syscall1(Syscall::ThreadExit, reason as usize);
    crate::arch::unreachable();
}

pub fn vmo_create(isolate: HandleId, len: usize) -> Result<HandleId, ErrorCode> {
    let ret = syscall2(Syscall::VmoCreate, isolate.as_usize(), len)?;
    Ok(HandleId::new(ret))
}
