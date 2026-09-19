use ftl_types::error::ErrorCode;
use ftl_types::thread::SyscallRegs;

use crate::address::UAddr;
use crate::address::USlice;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::Thread;

const CHUNK_LEN: usize = 256;

// TODO: Implement entropy pool and avoid relying on RDRAND.

pub fn sys_random_read(
    _current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let len = ctx.a1;
    if len == 0 {
        return Ok(SyscallOutput::Done(0));
    }

    let out = USlice::new(UAddr::new(ctx.a0), len)?;
    let mut tmp = [0u8; CHUNK_LEN];
    let mut offset = 0;
    while offset < len {
        let n = core::cmp::min(CHUNK_LEN, len - offset);
        crate::arch::random_read(&mut tmp[..n]);
        out.subslice(offset, n)?.write_bytes(&tmp[..n])?;
        offset += n;
    }

    Ok(SyscallOutput::Done(0))
}
