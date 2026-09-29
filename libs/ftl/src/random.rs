use ftl_types::error::ErrorCode;
use ftl_types::syscall::Syscall;

use crate::arch::syscall2;

pub fn read(buf: &mut [u8]) -> Result<(), ErrorCode> {
    let mut offset = 0;
    while offset < buf.len() {
        let rest = &mut buf[offset..];
        offset += syscall2(Syscall::RandomRead, rest.as_mut_ptr() as usize, rest.len())?;
    }

    Ok(())
}
