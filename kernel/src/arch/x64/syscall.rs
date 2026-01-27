use core::arch::naked_asm;
use core::mem::offset_of;

use crate::arch::Thread;
use crate::cpuvar::CpuVar;

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
