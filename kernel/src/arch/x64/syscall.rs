use core::arch::naked_asm;
use core::mem::offset_of;

use ftl_types::sink::EventBody;
use ftl_types::sink::EventType;
use ftl_types::sink::SyscallEvent;

use crate::arch::Thread;
use crate::cpuvar::CpuVar;
use crate::thread::Promise;
use crate::thread::return_to_user;

#[unsafe(naked)]
pub extern "C" fn direct_syscall_handler(
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    n: usize,
) -> usize {
    naked_asm!(
        "cli",
        "swapgs",

        // thread = CpuVar.current_thread
        "mov rax, gs:[{current_thread_offset}]",

        // Save the return address. Pop it since we'll return to the address
        // directly using IRETQ, not via RET.
        "pop r11",
        "mov [rax + {rip_offset}], r11",

        // Save rflags.
        "pushfq",
        "pop r11",
        "mov [rax + {rflags_offset}], r11",

        // Save callee-saved registers to the thread.
        "mov [rax + {rbx_offset}], rbx",
        "mov [rax + {rsp_offset}], rsp",
        "mov [rax + {rbp_offset}], rbp",
        "mov [rax + {r12_offset}], r12",
        "mov [rax + {r13_offset}], r13",
        "mov [rax + {r14_offset}], r14",
        "mov [rax + {r15_offset}], r15",

        "call {syscall_handler}",
        current_thread_offset = const offset_of!(CpuVar, current_thread),
        rip_offset = const offset_of!(Thread, rip),
        rflags_offset = const offset_of!(Thread, rflags),
        rbx_offset = const offset_of!(Thread, rbx),
        rsp_offset = const offset_of!(Thread, rsp),
        rbp_offset = const offset_of!(Thread, rbp),
        r12_offset = const offset_of!(Thread, r12),
        r13_offset = const offset_of!(Thread, r13),
        r14_offset = const offset_of!(Thread, r14),
        r15_offset = const offset_of!(Thread, r15),
        syscall_handler = sym crate::syscall::syscall_handler,
    )
}

/// The handler for SYSCALL instruction in sandboxed threads.
///
/// - RCX: The return address
/// - R11: RFLAGS
#[unsafe(naked)]
pub extern "C" fn sandboxed_syscall_handler() -> ! {
    naked_asm!(
        "cli",
        "swapgs",
        // Save syscall number.
        "mov [gs:{scratch_offset}], rax",

        // thread = CpuVar.current_thread
        "mov rax, gs:[{current_thread_offset}]",

        // Save the context.
        "mov [rax + {rip_offset}], rcx",
        "mov [rax + {rflags_offset}], r11",
        "mov [rax + {rbx_offset}], rbx",
        "mov [rax + {rcx_offset}], rcx",
        "mov [rax + {rdx_offset}], rdx",
        "mov [rax + {rdi_offset}], rdi",
        "mov [rax + {rsi_offset}], rsi",
        "mov [rax + {rsp_offset}], rsp",
        "mov [rax + {rbp_offset}], rbp",
        "mov [rax + {r8_offset}], r8",
        "mov [rax + {r9_offset}], r9",
        "mov [rax + {r10_offset}], r10",
        "mov [rax + {r11_offset}], r11",
        "mov [rax + {r12_offset}], r12",
        "mov [rax + {r13_offset}], r13",
        "mov [rax + {r14_offset}], r14",
        "mov [rax + {r15_offset}], r15",

        // Restore the user RAX.
        "mov rdi, [gs:{scratch_offset}]",
        "mov [rax + {rax_offset}], rdi",

        // Switch to the kernel stack.
        "mov rsp, gs:[{kernel_rsp_offset}]",

        "call {handle_sandboxed_syscall}",
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
        r8_offset = const offset_of!(Thread, r8),
        r9_offset = const offset_of!(Thread, r9),
        r10_offset = const offset_of!(Thread, r10),
        r11_offset = const offset_of!(Thread, r11),
        rsp_offset = const offset_of!(Thread, rsp),
        rbp_offset = const offset_of!(Thread, rbp),
        r12_offset = const offset_of!(Thread, r12),
        r13_offset = const offset_of!(Thread, r13),
        r14_offset = const offset_of!(Thread, r14),
        r15_offset = const offset_of!(Thread, r15),
        handle_sandboxed_syscall = sym handle_sandboxed_syscall,
    )
}

extern "C" fn handle_sandboxed_syscall() -> ! {
    let cpuvar = super::get_cpuvar();
    let arch_thread = cpuvar.current_thread.arch_thread();
    let thread = cpuvar.current_thread.thread();

    let regs = unsafe {
        SyscallEvent {
            rax: (*arch_thread).rax as u64,
            rdi: (*arch_thread).rdi as u64,
            rsi: (*arch_thread).rsi as u64,
            rdx: (*arch_thread).rdx as u64,
            r10: (*arch_thread).r10 as u64,
            r8: (*arch_thread).r8 as u64,
            r9: (*arch_thread).r9 as u64,
        }
    };

    thread.block_on_sandboxed_syscall(regs);
    return_to_user();
}
