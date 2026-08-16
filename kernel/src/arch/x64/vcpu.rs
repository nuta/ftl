use core::arch::asm;
use core::mem::offset_of;

use ftl_types::vcpu::SyscallRegs;

use super::gdt::GDT_USER_CS;
use super::gdt::GDT_USER_DS;

#[derive(Default, Debug)]
#[repr(C, packed)]
pub struct VCpu {
    // IRET frame. The order is important!
    pub(super) rip: u64,
    pub(super) cs: u64,
    pub(super) rflags: u64,
    pub(super) rsp: u64,
    pub(super) ss: u64,
    // Other registers.
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
    pub(super) gsbase: u64,
    pub(super) fsbase: u64,
}

impl VCpu {
    pub fn new(pc: usize, sp: usize) -> Self {
        Self {
            cs: GDT_USER_CS as u64,
            rflags: 0x202, // interrupts enabled
            ss: GDT_USER_DS as u64,
            rip: pc as u64,
            rsp: sp as u64,
            ..Default::default()
        }
    }

    pub fn get_syscall_regs(&self) -> SyscallRegs {
        SyscallRegs {
            n: self.rax as usize,
            a0: self.rdi as usize,
            a1: self.rsi as usize,
            a2: self.rdx as usize,
            a3: self.r10 as usize,
            a4: self.r8 as usize,
        }
    }

    pub fn set_syscall_retval(&mut self, retval: usize) {
        self.rax = retval as u64;
    }

    pub fn enter(vcpu: *const VCpu) -> ! {
        unsafe {
            asm!(
                "mov rsp, {}",
                "swapgs",
                "mov rax, [rsp + {gsbase_offset}]",
                "wrgsbase rax",
                "mov rax, [rsp + {fsbase_offset}]",
                "wrfsbase rax",
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
                // The RSP points to the beginning of *const VCpu, which is
                // the beginning of an IRET stack frame.
                //
                // The instruction will restore RIP, RFLAGS, RSP, and segment
                // registers (CS and SS), which means it jumps to the user's code
                // and switches to the user's stack, at once.
                //
                // > IRET pops SS:RSP unconditionally off the interrupt stack frame
                // > only when it is executed in 64-bit mode
                // >
                // > 7.14.3 IRET in IA-32e Mode
                "iretq",
                in(reg) vcpu,
                gsbase_offset = const offset_of!(VCpu, gsbase),
                fsbase_offset = const offset_of!(VCpu, fsbase),
                rax_offset = const offset_of!(VCpu, rax),
                rbx_offset = const offset_of!(VCpu, rbx),
                rcx_offset = const offset_of!(VCpu, rcx),
                rdx_offset = const offset_of!(VCpu, rdx),
                rsi_offset = const offset_of!(VCpu, rsi),
                rdi_offset = const offset_of!(VCpu, rdi),
                rbp_offset = const offset_of!(VCpu, rbp),
                r8_offset = const offset_of!(VCpu, r8),
                r9_offset = const offset_of!(VCpu, r9),
                r10_offset = const offset_of!(VCpu, r10),
                r11_offset = const offset_of!(VCpu, r11),
                r12_offset = const offset_of!(VCpu, r12),
                r13_offset = const offset_of!(VCpu, r13),
                r14_offset = const offset_of!(VCpu, r14),
                r15_offset = const offset_of!(VCpu, r15),
                options(noreturn)
            );
        }
    }
}
