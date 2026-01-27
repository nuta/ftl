use core::arch::asm;
use core::mem::offset_of;

use crate::arch::x64::boot::GDT_KERNEL_CS;

#[derive(Default)]
#[repr(C, packed)]
pub struct Thread {
    // IRET frame. The order is important!
    pub(super) rip: u64,
    pub(super) cs: u64,
    pub(super) rflags: u64,
    pub(super) rsp: u64,
    pub(super) ss: u64,
    // Other general-purpose registers.
    pub(super) rax: u64,
    pub(super) rbx: u64,
    pub(super) rcx: u64,
    pub(super) rdx: u64,
    pub(super) rsi: u64,
    pub(super) rdi: u64,
    pub(super) rbp: u64,
    pub(super) r8: u64,
    pub(super) r9: u64,
    pub(super) r10: u64,
    pub(super) r11: u64,
    pub(super) r12: u64,
    pub(super) r13: u64,
    pub(super) r14: u64,
    pub(super) r15: u64,
}

impl Thread {
    pub fn new(entry: usize, sp: usize, arg: usize) -> Self {
        Self {
            rip: entry as u64,
            cs: GDT_KERNEL_CS as u64,
            rflags: 0x2, // interrupts disabled
            rsp: sp as u64,
            rdi: arg as u64,
            ..Default::default()
        }
    }

    pub fn new_idle() -> Self {
        Self {
            ..Default::default()
        }
    }
}

pub fn thread_switch(thread: *const Thread) -> ! {
    unsafe {
        println!("switching to #{:x}", unsafe { thread as usize });
        asm!(
            "mov rsp, {}",
            "mov rax, [rsp + {rax_offset}]",
            "mov rbx, [rsp + {rbx_offset}]",
            "mov rcx, [rsp + {rcx_offset}]",
            "mov rdx, [rsp + {rdx_offset}]",
            "mov rsi, [rsp + {rsi_offset}]",
            "mov rdi, [rsp + {rdi_offset}]",
            "mov rbp, [rsp + {rbp_offset}]",
            "mov r8,  [rsp + {r8_offset}]",
            "mov r9,  [rsp + {r9_offset}]",
            "mov r10, [rsp + {r10_offset}]",
            "mov r11, [rsp + {r11_offset}]",
            "mov r12, [rsp + {r12_offset}]",
            "mov r13, [rsp + {r13_offset}]",
            "mov r14, [rsp + {r14_offset}]",
            "mov r15, [rsp + {r15_offset}]",
            "swapgs",
            "iretq",
            in(reg) thread,
            rax_offset = const offset_of!(Thread, rax),
            rbx_offset = const offset_of!(Thread, rbx),
            rcx_offset = const offset_of!(Thread, rcx),
            rdx_offset = const offset_of!(Thread, rdx),
            rsi_offset = const offset_of!(Thread, rsi),
            rdi_offset = const offset_of!(Thread, rdi),
            rbp_offset = const offset_of!(Thread, rbp),
            r8_offset = const offset_of!(Thread, r8),
            r9_offset = const offset_of!(Thread, r9),
            r10_offset = const offset_of!(Thread, r10),
            r11_offset = const offset_of!(Thread, r11),
            r12_offset = const offset_of!(Thread, r12),
            r13_offset = const offset_of!(Thread, r13),
            r14_offset = const offset_of!(Thread, r14),
            r15_offset = const offset_of!(Thread, r15),
            options(noreturn)
        );
    }
}
