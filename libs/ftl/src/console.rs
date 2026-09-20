use ftl_types::error::ErrorCode;
use ftl_types::syscall::Syscall;

use crate::arch::syscall1;
use crate::arch::syscall2;
use crate::poll::Poll;

pub fn write(mut bytes: &[u8]) -> Result<(), ErrorCode> {
    while !bytes.is_empty() {
        let written = syscall2(Syscall::Print, bytes.as_ptr() as usize, bytes.len())?;
        if written == 0 {
            break;
        }
        bytes = &bytes[written..];
    }
    Ok(())
}

pub fn read(buf: &mut [u8]) -> Result<usize, ErrorCode> {
    syscall2(Syscall::ConsoleRead, buf.as_mut_ptr() as usize, buf.len())
}

pub fn subscribe(poll: &Poll) -> Result<(), ErrorCode> {
    syscall1(Syscall::ConsoleSubscribe, poll.handle().id().as_usize())?;
    Ok(())
}
