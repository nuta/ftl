use core::arch::asm;
use core::arch::naked_asm;
use core::mem::offset_of;

use super::gdt::GDT_KERNEL_CS;

unsafe fn rdmsr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") low,
            out("edx") high,
        );
    }
    ((high as u64) << 32) | (low as u64)
}

unsafe fn wrmsr(msr: u32, value: u64) {
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") value as u32,
            in("edx") (value >> 32) as u32,
        );
    }
}

fn handle_syscall() -> ! {
    panic!("syscall handler not implemented");
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
extern "C" fn syscall_handler() -> ! {
    naked_asm!(
        "cli",
        "swapgs",
        "cld",

        // TODO: Save registers.
        "mov rsp, gs:[{kernel_rsp_offset}]",

        "jmp {handle_syscall}",
        handle_syscall = sym handle_syscall,
        kernel_rsp_offset = const offset_of!(crate::cpuvar::CpuVar, arch.kernel_rsp),
    );
}

pub(super) fn init() {
    const MSR_IA32_STAR: u32 = 0xc000_0081;
    const MSR_IA32_LSTAR: u32 = 0xc000_0082;
    const MSR_IA32_FMASK: u32 = 0xc000_0084;
    const MSR_IA32_EFER: u32 = 0xc000_0080;
    const EFER_SCE: u64 = 1 << 0;
    const SYSCALL_FMASK: u64 = 1 << 9; // Clear IF on SYSCALL entry.

    // Configure SYSCALL instructions. SYSRET (STAR[63:48]) is not set because
    // we always use IRET.
    unsafe {
        let syscall_handler = syscall_handler as *const () as u64;
        wrmsr(MSR_IA32_EFER, rdmsr(MSR_IA32_EFER) | EFER_SCE);
        wrmsr(MSR_IA32_STAR, (GDT_KERNEL_CS as u64) << 32);
        wrmsr(MSR_IA32_LSTAR, syscall_handler);
        wrmsr(MSR_IA32_FMASK, SYSCALL_FMASK);
    }
}
