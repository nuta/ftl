use core::arch::asm;
use core::mem::MaybeUninit;
use core::mem::offset_of;

use ftl_types::error::ErrorCode;
use ftl_types::thread::Regs;
use ftl_types::thread::RegsKind;
use ftl_types::thread::SyscallRegs;

use super::gdt::GDT_USER_CS;
use super::gdt::GDT_USER_DS;
use crate::address::USlice;
use crate::arch;
use crate::arch::MIN_PAGE_SIZE;
use crate::arch::USER_ADDR_END;
use crate::memory::PAGE_ALLOCATOR;
use crate::memory::PageType;

pub(super) const XSTATE_MASK: u64 = (1 << 0) | (1 << 1); // x87 | SSE

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
    pub(super) xsave_ptr: u64,
    pub(super) fault_pc: u64,
    pub(super) cookie: u64,
}

impl Thread {
    pub fn new(pc: usize, sp: usize, fault_pc: usize, cookie: usize) -> Result<Self, ErrorCode> {
        let paddr = PAGE_ALLOCATOR
            .alloc(MIN_PAGE_SIZE, PageType::Zeroed)
            .ok_or(ErrorCode::OutOfMemory)?;

        Ok(Self {
            cs: GDT_USER_CS as u64,
            rflags: 0x202, // interrupts enabled
            ss: GDT_USER_DS as u64,
            rip: pc as u64,
            rsp: sp as u64,
            fault_pc: fault_pc as u64,
            xsave_ptr: arch::paddr2vaddr(paddr).as_usize() as u64,
            cookie: cookie as u64,
            ..Default::default()
        })
    }

    pub fn get_syscall_regs(&self) -> SyscallRegs {
        SyscallRegs {
            n: self.rax as usize,
            a0: self.rdi as usize,
            a1: self.rsi as usize,
            a2: self.rdx as usize,
            a3: self.r10 as usize,
            a4: self.r8 as usize,
            a5: self.r9 as usize,
        }
    }

    pub fn set_syscall_retval(&mut self, retval: usize) {
        self.rax = retval as u64;
    }

    pub fn write_regs(&mut self, kind: RegsKind, uslice: USlice) -> Result<(), ErrorCode> {
        let mut regs = MaybeUninit::<Regs>::uninit();
        let regs = unsafe { uslice.read_uninit(&mut regs)? };

        match kind {
            RegsKind::FsBase => {
                let fs_base = unsafe { regs.fs_base };
                if fs_base >= USER_ADDR_END {
                    return Err(ErrorCode::InvalidArg);
                }

                self.fsbase = fs_base as u64;
            }
            RegsKind::FpAndVector => return Err(ErrorCode::Unsupported),
        }

        Ok(())
    }

    pub fn copy_regs(&mut self, src: &Self, kind: RegsKind) -> Result<(), ErrorCode> {
        match kind {
            RegsKind::FsBase => self.fsbase = src.fsbase,
            RegsKind::FpAndVector => unsafe {
                core::ptr::copy_nonoverlapping(
                    src.xsave_ptr as *const u8,
                    self.xsave_ptr as *mut u8,
                    MIN_PAGE_SIZE, // FIXME: use the correct size
                );
            },
        }

        Ok(())
    }

    pub fn enter(thread: *const Thread) -> ! {
        // IRETQ causes an exception in kernel mode if RIP or RSP is non-canonical.
        let t = unsafe { &*thread };
        if t.rip as usize >= USER_ADDR_END || t.rsp as usize >= USER_ADDR_END {
            // Exit the current thread.
            {
                let cpuvar = super::get_cpuvar();
                if let Some(current) = cpuvar.current_thread.thread() {
                    if let Err(e) = current.exit() {
                        warn!("failed to exit the thread on bad IRETQ: {:?}", e);
                    }
                }
            }

            crate::scheduler::return_to_user();
        }

        unsafe {
            asm!(
                "mov rsp, {}",
                "mov rdi, [rsp + {xsave_ptr_offset}]",
                "mov eax, {xstate_mask_lo}",
                "mov edx, {xstate_mask_hi}",
                "xrstor64 [rdi]",
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
                xstate_mask_lo = const XSTATE_MASK & 0xffff_ffff,
                xstate_mask_hi = const XSTATE_MASK >> 32,
                xsave_ptr_offset = const offset_of!(Thread, xsave_ptr),
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
