use ftl_types::error::ErrorCode;
use ftl_types::syscall::Syscall;

use crate::arch::syscall2;

pub fn read(buf: &mut [u8]) -> Result<(), ErrorCode> {
    syscall2(Syscall::RandomRead, buf.as_mut_ptr() as usize, buf.len())?;
    Ok(())
}
