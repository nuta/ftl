use crate::types::c_int;

pub const NSIG: c_int = 65;

pub const SIG_DFL: usize = 0;
pub const SIG_IGN: usize = 1;
pub const SIGILL: c_int = 4;
pub const SIGTRAP: c_int = 5;
pub const SIGBUS: c_int = 7;
pub const SIGFPE: c_int = 8;
pub const SIGKILL: c_int = 9;
pub const SIGSEGV: c_int = 11;
pub const SIGPIPE: c_int = 13;
pub const SIGCHLD: c_int = 17;
pub const SIGSTOP: c_int = 19;

pub const SA_ONSTACK: usize = 0x0800_0000;
pub const SA_NODEFER: usize = 0x4000_0000;
pub const SA_RESETHAND: usize = 0x8000_0000;

pub const SIG_BLOCK: c_int = 0;
pub const SIG_UNBLOCK: c_int = 1;
pub const SIG_SETMASK: c_int = 2;

pub const SS_ONSTACK: c_int = 1;
pub const SS_DISABLE: c_int = 2;
pub const SS_AUTODISARM: c_int = 1 << 31;

// The best value is unclear to me. 2048 bytes should be enough, I guess.
pub const MINSIGSTKSZ: usize = 2048;

pub const SI_USER: c_int = 0;
pub const SI_KERNEL: c_int = 0x80;
pub const ILL_ILLOPN: c_int = 2;
pub const FPE_INTDIV: c_int = 1;
pub const SEGV_MAPERR: c_int = 1;
pub const SEGV_ACCERR: c_int = 2;
pub const BUS_ADRALN: c_int = 1;
pub const TRAP_BRKPT: c_int = 1;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct SigAction {
    pub handler: usize,
    pub flags: usize,
    pub restorer: usize,
    pub mask: u64,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct StackT {
    pub sp: usize,
    pub flags: c_int,
    pub size: usize,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct SigInfo {
    pub signo: c_int,
    pub errno: c_int,
    pub code: c_int,
    pub addr: usize,
    pub _pad: [u8; 104],
}

impl SigInfo {
    pub const fn new(signo: c_int, code: c_int, addr: usize) -> Self {
        Self {
            signo,
            errno: 0,
            code,
            addr,
            _pad: [0; 104],
        }
    }
}

ftl_utils::static_assert!(size_of::<StackT>() == 24);
ftl_utils::static_assert!(size_of::<SigInfo>() == 128);
