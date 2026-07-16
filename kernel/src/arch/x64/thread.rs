use core::arch::asm;
use core::mem::offset_of;

use ftl_api::thread::ContextData;
use ftl_api::thread::ContextKind;
use ftl_api::thread::FsBase;
use ftl_api::thread::InitRegs;
use ftl_api::thread::SyscallArgs;
use ftl_api::thread::Sysret;

use super::gdt::GDT_USER_CS;
use super::gdt::GDT_USER_DS;

#[derive(Default, Debug)]
#[repr(C, packed)]
pub struct Thread {
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

impl Thread {
    pub fn new() -> Self {
        Self {
            cs: GDT_USER_CS as u64,
            rflags: 0x202, // interrupts enabled
            ss: GDT_USER_DS as u64,
            ..Default::default()
        }
    }

    pub fn read_context(&self, kind: ContextKind, regs: &mut ContextData) {
        match kind {
            ContextKind::SyscallArgs => {
                regs.syscall_args = SyscallArgs {
                    n: self.rax,
                    arg0: self.rdi,
                    arg1: self.rsi,
                    arg2: self.rdx,
                    arg3: self.r10,
                    arg4: self.r8,
                    arg5: self.r9,
                };
            }
            ContextKind::Sysret => {
                regs.sysret = Sysret { retval: self.rax };
            }
            ContextKind::InitRegs => {
                regs.init_regs = InitRegs {
                    pc: self.rip,
                    sp: self.rsp,
                };
            }
            ContextKind::Fsbase => {
                regs.fsbase = FsBase { base: self.fsbase };
            }
        }
    }

    pub fn write_context(&mut self, kind: ContextKind, regs: &ContextData) {
        match kind {
            ContextKind::SyscallArgs => {
                let args = unsafe { regs.syscall_args };
                self.rax = args.n;
                self.rdi = args.arg0;
                self.rsi = args.arg1;
                self.rdx = args.arg2;
                self.r10 = args.arg3;
                self.r8 = args.arg4;
                self.r9 = args.arg5;
            }
            ContextKind::Sysret => {
                self.rax = unsafe { regs.sysret.retval };
            }
            ContextKind::InitRegs => {
                let init_regs = unsafe { regs.init_regs };
                self.rip = init_regs.pc;
                self.rsp = init_regs.sp;
            }
            ContextKind::Fsbase => {
                self.fsbase = unsafe { regs.fsbase }.base;
            }
        }
    }

    pub fn enter(thread: *const Thread) -> ! {
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
                // The RSP points to the beginning of *const Thread, which is
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
                in(reg) thread,
                gsbase_offset = const offset_of!(Thread, gsbase),
                fsbase_offset = const offset_of!(Thread, fsbase),
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
}
