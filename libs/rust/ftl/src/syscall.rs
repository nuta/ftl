use ftl_types::syscall::SYS_CONSOLE_WRITE;

use crate::arch::get_start_info;

#[inline(always)]
fn syscall2(n: usize, a0: usize, a1: usize) -> usize {
    let start_info = get_start_info();
    (start_info.syscall)(a0, a1, 0, 0, 0, n)
}

pub fn sys_console_write(s: &[u8]) {
    syscall2(SYS_CONSOLE_WRITE, s.as_ptr() as usize, s.len());
}
