use ftl_types::syscall::Syscall;
use ftl_types::vcpu::ExitReason;

fn syscall1(n: Syscall, a0: usize) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use core::arch::asm;

        asm!(
            "syscall",
            in("rax") n as usize,
            in("rdi") a0,
            out("rcx") _,
            out("r11") _,
        );
    }
}

fn syscall2(n: Syscall, a0: usize, a1: usize) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use core::arch::asm;

        asm!(
            "syscall",
            in("rax") n as usize,
            in("rdi") a0,
            in("rsi") a1,
            out("rcx") _,
            out("r11") _,
        );
    }
}

pub fn print(bytes: &[u8]) {
    crate::syscall::syscall2(Syscall::Print, bytes.as_ptr() as usize, bytes.len());
}

pub fn vcpu_exit(reason: ExitReason) -> ! {
    crate::syscall::syscall1(Syscall::VCpuExit, reason as usize);
    crate::arch::unreachable();
}
