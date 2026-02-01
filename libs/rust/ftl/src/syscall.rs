use ftl_types::error::ErrorCode;
use ftl_types::syscall::ERROR_RETVAL_BASE;
use ftl_types::syscall::SYS_CONSOLE_WRITE;
use ftl_types::syscall::SYS_X64_IOPL;

use crate::arch::get_start_info;

#[inline(always)]
fn syscall(
    n: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
) -> Result<usize, ErrorCode> {
    let start_info = get_start_info();
    let result = (start_info.syscall)(a0, a1, a2, a3, a4, n);
    if let Some(error) = result.checked_sub(ERROR_RETVAL_BASE) {
        Err(ErrorCode::from(error))
    } else {
        Ok(result)
    }
}

#[inline(always)]
pub(super) fn syscall0(n: usize) -> Result<usize, ErrorCode> {
    syscall(n, 0, 0, 0, 0, 0)
}

#[inline(always)]
pub(super) fn syscall1(n: usize, a0: usize) -> Result<usize, ErrorCode> {
    syscall(n, a0, 0, 0, 0, 0)
}

#[inline(always)]
pub(super) fn syscall2(n: usize, a0: usize, a1: usize) -> Result<usize, ErrorCode> {
    syscall(n, a0, a1, 0, 0, 0)
}

#[inline(always)]
pub(super) fn syscall3(n: usize, a0: usize, a1: usize, a2: usize) -> Result<usize, ErrorCode> {
    syscall(n, a0, a1, a2, 0, 0)
}

#[inline(always)]
pub(super) fn syscall4(
    n: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
) -> Result<usize, ErrorCode> {
    syscall(n, a0, a1, a2, a3, 0)
}

#[inline(always)]
pub(super) fn syscall5(
    n: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
) -> Result<usize, ErrorCode> {
    syscall(n, a0, a1, a2, a3, a4)
}

pub fn sys_console_write(s: &[u8]) {
    let _ = syscall2(SYS_CONSOLE_WRITE, s.as_ptr() as usize, s.len());
}

pub fn sys_x64_iopl(enable: bool) -> Result<usize, ErrorCode> {
    syscall1(SYS_X64_IOPL, enable as usize)
}
