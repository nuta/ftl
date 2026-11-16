use core::arch::asm;
use core::mem::offset_of;

use ftl_types::thread::X64_USER_CS;
use ftl_types::thread::X64_USER_SS;
use ftl_utils::alignment::align_down;

use super::SyscallFrame;
use crate::signal::SignalSet;
use crate::types::signal::SigInfo;
use crate::types::signal::StackT;

const USER_RFLAGS: usize = 0x202;
const SIGRETURN_RFLAGS_MASK: usize = 0x50dd5;
const RED_ZONE_SIZE: usize = 128;

/// `mcontext_t` struct.
#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct UserContext {
    r8: usize,
    r9: usize,
    r10: usize,
    r11: usize,
    r12: usize,
    r13: usize,
    r14: usize,
    r15: usize,
    rdi: usize,
    rsi: usize,
    rbp: usize,
    rbx: usize,
    rdx: usize,
    rax: usize,
    rcx: usize,
    rsp: usize,
    rip: usize,
    rflags: usize,
    cs: u16,
    gs: u16,
    fs: u16,
    ss: u16,
    err: usize,
    trapno: usize,
    oldmask: usize,
    cr2: usize,
    fpstate: usize,
    reserved: [usize; 8],
}

/// `ucontext_t` struct.
#[repr(C)]
struct UContext {
    flags: usize,
    link: usize,
    stack: StackT,
    mcontext: UserContext,
    sigmask: SignalSet,
}

#[repr(C)]
struct RtSigFrame {
    restorer: usize,
    uc: UContext,
    info: SigInfo,
}

impl UserContext {
    pub fn from_syscall_frame(frame: &SyscallFrame) -> Self {
        Self {
            r8: frame.r8,
            r9: frame.r9,
            r10: frame.r10,
            r11: USER_RFLAGS,
            r12: frame.r12,
            r13: frame.r13,
            r14: frame.r14,
            r15: frame.r15,
            rdi: frame.rdi,
            rsi: frame.rsi,
            rbp: frame.rbp,
            rbx: frame.rbx,
            rdx: frame.rdx,
            rax: frame.rax,
            rcx: frame.rip,
            rsp: frame.rsp,
            rip: frame.rip,
            rflags: USER_RFLAGS,
            cs: X64_USER_CS as u16,
            ss: X64_USER_SS as u16,
            ..Default::default()
        }
    }

    pub fn sp(&self) -> usize {
        self.rsp
    }

    pub unsafe fn enter_signal(
        &mut self,
        sp: usize,
        handler: usize,
        restorer: usize,
        info: SigInfo,
        stack: StackT,
        sigmask: SignalSet,
    ) {
        let sp = sp.wrapping_sub(RED_ZONE_SIZE + size_of::<RtSigFrame>());
        let frame = align_down(sp, 16).wrapping_sub(8);

        let mut mcontext = *self;
        mcontext.oldmask = sigmask.to_raw() as usize;
        mcontext.cr2 = info.addr;
        mcontext.fpstate = 0;

        unsafe {
            (frame as *mut RtSigFrame).write(RtSigFrame {
                restorer,
                uc: UContext {
                    flags: 0,
                    link: 0,
                    stack,
                    mcontext,
                    sigmask,
                },
                info,
            });
        }

        self.rdi = info.signo as usize;
        self.rsi = frame + offset_of!(RtSigFrame, info);
        self.rdx = frame + offset_of!(RtSigFrame, uc);
        self.rax = 0;
        self.rsp = frame;
        self.rip = handler;
        self.rflags = USER_RFLAGS;
    }

    pub unsafe fn return_from_signal(sp: usize) -> (Self, SignalSet) {
        let uc = unsafe { (sp as *const UContext).read() };
        let mut ctx = uc.mcontext;
        ctx.rflags = (ctx.rflags & SIGRETURN_RFLAGS_MASK) | USER_RFLAGS;
        (ctx, uc.sigmask)
    }

    pub fn restore(&self) -> ! {
        unsafe {
            asm!(
                // Build an IRET frame.
                "push {user_ss}",
                "push qword ptr [rdi + {rsp}]",
                "push qword ptr [rdi + {rflags}]",
                "push {user_cs}",
                "push qword ptr [rdi + {rip}]",

                // Restore general-purpose registers.
                "mov r8, [rdi + {r8}]",
                "mov r9, [rdi + {r9}]",
                "mov r10, [rdi + {r10}]",
                "mov r11, [rdi + {r11}]",
                "mov r12, [rdi + {r12}]",
                "mov r13, [rdi + {r13}]",
                "mov r14, [rdi + {r14}]",
                "mov r15, [rdi + {r15}]",
                "mov rsi, [rdi + {rsi}]",
                "mov rbp, [rdi + {rbp}]",
                "mov rbx, [rdi + {rbx}]",
                "mov rdx, [rdi + {rdx}]",
                "mov rax, [rdi + {rax}]",
                "mov rcx, [rdi + {rcx}]",
                "mov rdi, [rdi + {rdi}]",

                // Restore user RIP, RFLAGS, and RSP at once.
                "iretq",
                user_cs = const X64_USER_CS,
                user_ss = const X64_USER_SS,
                r8 = const offset_of!(UserContext, r8),
                r9 = const offset_of!(UserContext, r9),
                r10 = const offset_of!(UserContext, r10),
                r11 = const offset_of!(UserContext, r11),
                r12 = const offset_of!(UserContext, r12),
                r13 = const offset_of!(UserContext, r13),
                r14 = const offset_of!(UserContext, r14),
                r15 = const offset_of!(UserContext, r15),
                rdi = const offset_of!(UserContext, rdi),
                rsi = const offset_of!(UserContext, rsi),
                rbp = const offset_of!(UserContext, rbp),
                rbx = const offset_of!(UserContext, rbx),
                rdx = const offset_of!(UserContext, rdx),
                rax = const offset_of!(UserContext, rax),
                rcx = const offset_of!(UserContext, rcx),
                rsp = const offset_of!(UserContext, rsp),
                rip = const offset_of!(UserContext, rip),
                rflags = const offset_of!(UserContext, rflags),
                in("rdi") self,
                options(noreturn),
            )
        }
    }
}
