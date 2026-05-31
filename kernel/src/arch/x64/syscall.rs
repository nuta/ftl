use core::arch::naked_asm;
use core::mem::offset_of;

use super::gdt::GDT_KERNEL_CS;
use super::msr::rdmsr;
use super::msr::wrmsr;
use super::thread::Thread;
use crate::cpuvar::CpuVar;

#[unsafe(naked)]
#[unsafe(no_mangle)]
extern "C" fn syscall_handler() -> ! {
    naked_asm!(
        "cli",
        "swapgs",
        "cld",

        // Save RAX temporarily.
        "mov gs:[{scratch_offset}], rax",

        // Save registers.
        "mov rax, gs:[{current_thread_offset}]",
        "mov [rax + {rip_offset}], rcx",
        "mov [rax + {rflags_offset}], r11",
        "mov [rax + {rbx_offset}], rbx",
        "mov [rax + {rcx_offset}], rcx",
        "mov [rax + {rdx_offset}], rdx",
        "mov [rax + {rdi_offset}], rdi",
        "mov [rax + {rsi_offset}], rsi",
        "mov [rax + {rbp_offset}], rbp",
        "mov [rax + {r8_offset}], r8",
        "mov [rax + {r9_offset}], r9",
        "mov [rax + {r10_offset}], r10",
        "mov [rax + {r11_offset}], r11",
        "mov [rax + {r12_offset}], r12",
        "mov [rax + {r13_offset}], r13",
        "mov [rax + {r14_offset}], r14",
        "mov [rax + {r15_offset}], r15",
        "mov [rax + {rsp_offset}], rsp",

        // Save RAX to the thread struct.
        "mov rdi, gs:[{scratch_offset}]",
        "mov [rax + {rax_offset}], rdi",

        // Switch to the kernel stack.
        "mov rsp, gs:[{kernel_rsp_offset}]",

        // Call the syscall handler.
        "jmp {handle_syscall}",
        handle_syscall = sym crate::syscall::handle_syscall,
        current_thread_offset = const offset_of!(CpuVar, current_thread),
        scratch_offset = const offset_of!(CpuVar, arch.scratch),
        kernel_rsp_offset = const offset_of!(CpuVar, arch.kernel_rsp),
        rip_offset = const offset_of!(Thread, rip),
        rflags_offset = const offset_of!(Thread, rflags),
        rax_offset = const offset_of!(Thread, rax),
        rbx_offset = const offset_of!(Thread, rbx),
        rcx_offset = const offset_of!(Thread, rcx),
        rdx_offset = const offset_of!(Thread, rdx),
        rdi_offset = const offset_of!(Thread, rdi),
        rsi_offset = const offset_of!(Thread, rsi),
        rsp_offset = const offset_of!(Thread, rsp),
        rbp_offset = const offset_of!(Thread, rbp),
        r8_offset = const offset_of!(Thread, r8),
        r9_offset = const offset_of!(Thread, r9),
        r10_offset = const offset_of!(Thread, r10),
        r11_offset = const offset_of!(Thread, r11),
        r12_offset = const offset_of!(Thread, r12),
        r13_offset = const offset_of!(Thread, r13),
        r14_offset = const offset_of!(Thread, r14),
        r15_offset = const offset_of!(Thread, r15),
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
