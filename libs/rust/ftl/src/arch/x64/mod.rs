use core::arch::asm;

use ftl_types::error::ErrorCode;
use ftl_types::syscall::Syscall;

pub fn unreachable() -> ! {
    unsafe {
        asm!("ud2", options(noreturn));
    }
}

fn convert_retval(rax: usize) -> Result<usize, ErrorCode> {
    let ret = rax as isize;
    if ret < 0 {
        Err(ErrorCode::from_usize(rax))
    } else {
        Ok(rax)
    }
}

pub fn syscall1(n: Syscall, a0: usize) -> Result<usize, ErrorCode> {
    let mut rax = n as usize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") rax,
            in("rdi") a0,
            out("rcx") _,
            out("r11") _,
        );
    }
    convert_retval(rax)
}

pub fn syscall2(n: Syscall, a0: usize, a1: usize) -> Result<usize, ErrorCode> {
    let mut rax = n as usize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") rax,
            in("rdi") a0,
            in("rsi") a1,
            out("rcx") _,
            out("r11") _,
        );
    }
    convert_retval(rax)
}

pub fn syscall4(
    n: Syscall,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
) -> Result<usize, ErrorCode> {
    let mut rax = n as usize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") rax,
            in("rdi") a0,
            in("rsi") a1,
            in("rdx") a2,
            in("r10") a3,
            out("rcx") _,
            out("r11") _,
        );
    }
    convert_retval(rax)
}

pub fn syscall6(
    n: Syscall,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
) -> Result<usize, ErrorCode> {
    let mut rax = n as usize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") rax,
            in("rdi") a0,
            in("rsi") a1,
            in("rdx") a2,
            in("r10") a3,
            in("r8") a4,
            in("r9") a5,
            out("rcx") _,
            out("r11") _,
        );
    }
    convert_retval(rax)
}
