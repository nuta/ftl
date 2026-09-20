use ftl_types::error::ErrorCode;
use ftl_types::syscall::Syscall;

use crate::arch::syscall1;
use crate::arch::syscall2;
use crate::poll::Poll;

pub fn write(buf: &[u8]) -> Result<usize, ErrorCode> {
    let len = syscall2(Syscall::ConsoleWrite, buf.as_ptr() as usize, buf.len())?;
    Ok(len)
}

pub fn read(buf: &mut [u8]) -> Result<usize, ErrorCode> {
    syscall2(Syscall::ConsoleRead, buf.as_mut_ptr() as usize, buf.len())
}

pub fn subscribe(poll: &Poll) -> Result<(), ErrorCode> {
    syscall1(Syscall::ConsoleSubscribe, poll.handle().id().as_usize())?;
    Ok(())
}
