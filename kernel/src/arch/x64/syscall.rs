use core::arch::naked_asm;
use core::mem::offset_of;
use core::mem::size_of;

use ftl_types::syscall::SYSCALL_BASE;
use ftl_types::thread::SyscallFrame;
use ftl_utils::static_assert;

use super::gdt::GDT_KERNEL_CS;
use super::gdt::GDT_KERNEL_DS;
use super::gdt::GDT_USER_CS;
use super::gdt::GDT_USER_DS;
use super::msr::rdmsr;
use super::msr::wrmsr;
use super::thread::Thread;
use super::thread::XSTATE_MASK;
use super::vmspace::USER_ADDR_END;
use crate::cpuvar::CpuVar;
use crate::scheduler;

const USER_RFLAGS: u64 = 0x202;
const RED_ZONE_SIZE: usize = 128;

fn try_exit_current() {
    let cpuvar = super::get_cpuvar();
    let Some(thread) = cpuvar.current_thread.thread() else {
        return;
    };

    if let Err(e) = thread.exit() {
        warn!("failed to terminate thread: {:?}", e);
        return;
    }
}

extern "C" fn do_syscall_copy_recover() -> ! {
    try_exit_current();
    scheduler::return_to_user()
}

#[unsafe(naked)]
pub(super) extern "C" fn syscall_copy_recover() -> ! {
    naked_asm!(
        // Align the kernel stack to 16 bytes.
        //
        // We might not need this since we don't use SSE in kernel.
        "and rsp, -16",
        "call {do_syscall_copy_recover}",
        "int3", // unreachable
        do_syscall_copy_recover = sym do_syscall_copy_recover,
    )
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
extern "C" fn syscall_handler() -> ! {
    naked_asm!(
        "cli",
        "swapgs",
        "cld",

        // Is the syscall number in the FTL range? If not, jump to the
        // trampoline. Compare RAX as an unsigned usize value.
        "cmp rax, {ftl_syscall_base}",
        "jb 2f",

        // FTL system call.
        //
        // Save RAX temporarily, then save registers to the current thread's
        // context.
        "mov gs:[{scratch_offset}], rax",
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

        // Save the user XSTATE.
        "mov rdi, [rax + {xsave_ptr_offset}]",
        "mov eax, {xstate_mask_lo}",
        "mov edx, {xstate_mask_hi}",
        "xsaveopt64 [rdi]",

        // Switch to the kernel stack and handle the FTL syscall.
        "mov rsp, gs:[{kernel_rsp_offset}]",
        "jmp {handle_syscall}",

        // System call trampoline: Build the trap frame and jump back to the
        // userspace OS.
        "2:",
        "mov gs:[{scratch_offset}], rsp",
        "mov rsp, gs:[{kernel_rsp_offset}]", // Switch to the kernel stack.
        "push rcx", // user RIP
        "push r11", // user RFLAGS

        // Check if user RSP is safe to use. We're in kernel mode and need to
        // extremely carefully avoid reading/writing kernel memory on behalf of
        // the user!
        "mov r11, gs:[{scratch_offset}]", // user RSP
        "mov rcx, {user_addr_end}",
        "cmp r11, rcx",
        "jae {syscall_copy_recover}",

        // Allocate the trap frame (below the red zone) from user RSP.
        "sub r11, {syscall_frame_size}",
        "jc {syscall_copy_recover}", // user RSP is too low

        // Write user RFLAGS
        "mov rcx, [rsp]",
        ".global syscall_copy0; syscall_copy0:",
        "mov [r11 + {frame_rflags_offset}], rcx",

        // Write user RIP
        "mov rcx, [rsp + 8]",
        ".global syscall_copy1; syscall_copy1:",
        "mov [r11 + {frame_rip_offset}], rcx",

        // Write cookie
        "mov rcx, gs:[{current_thread_offset}]",
        "mov rcx, [rcx + {cookie_offset}]",
        ".global syscall_copy2; syscall_copy2:",
        "mov [r11 + {frame_cookie_offset}], rcx",

        // Load fault_pc from the current thread.
        "mov rcx, gs:[{current_thread_offset}]",
        "mov rcx, [rcx + {fault_pc_offset}]", // user RIP (when SYSRET-ing)
        "mov rsp, r11", // user RSP (for fault handler)
        "mov r11, {user_rflags}", // user RFLAGS (for fault handler)
        "swapgs",
        "sysretq",
        handle_syscall = sym crate::syscall::handle_syscall,
        syscall_copy_recover = sym syscall_copy_recover,
        ftl_syscall_base = const SYSCALL_BASE as isize,
        user_addr_end = const USER_ADDR_END,
        syscall_frame_size = const RED_ZONE_SIZE + size_of::<SyscallFrame>(),
        user_rflags = const USER_RFLAGS,
        current_thread_offset = const offset_of!(CpuVar, current_thread),
        xstate_mask_lo = const XSTATE_MASK & 0xffff_ffff,
        xstate_mask_hi = const XSTATE_MASK >> 32,
        xsave_ptr_offset = const offset_of!(Thread, xsave_ptr),
        fault_pc_offset = const offset_of!(Thread, fault_pc),
        cookie_offset = const offset_of!(Thread, cookie),
        frame_rflags_offset = const offset_of!(SyscallFrame, rflags),
        frame_rip_offset = const offset_of!(SyscallFrame, rip),
        frame_cookie_offset = const offset_of!(SyscallFrame, cookie),
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

const SYSCALL_SEG_BASE: u64 = GDT_KERNEL_CS as u64;
const SYSRET_SEG_BASE: u64 = (GDT_USER_CS as u64) - 16;

// SYSCALL/SYSRET computes SS from IA32_STAR.
static_assert!(SYSCALL_SEG_BASE + 8 == GDT_KERNEL_DS as u64); // kernel SS
static_assert!(SYSRET_SEG_BASE + 8 == GDT_USER_DS as u64); // user SS

pub(super) fn init() {
    const MSR_IA32_STAR: u32 = 0xc000_0081;
    const MSR_IA32_LSTAR: u32 = 0xc000_0082;
    const MSR_IA32_FMASK: u32 = 0xc000_0084;
    const MSR_IA32_EFER: u32 = 0xc000_0080;
    const EFER_SCE: u64 = 1 << 0;

    // RFLAGS bits to clear on SYSCALL entry.
    const SYSCALL_FMASK: u64 = (1 << 8) | (1 << 9); // TF | IF

    unsafe {
        let syscall_handler = syscall_handler as *const () as u64;
        wrmsr(MSR_IA32_EFER, rdmsr(MSR_IA32_EFER) | EFER_SCE);
        wrmsr(
            MSR_IA32_STAR,
            (SYSRET_SEG_BASE << 48) | (SYSCALL_SEG_BASE << 32),
        );
        wrmsr(MSR_IA32_LSTAR, syscall_handler);
        wrmsr(MSR_IA32_FMASK, SYSCALL_FMASK);
    }
}
