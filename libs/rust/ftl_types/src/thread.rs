#[repr(usize)]
pub enum ExitReason {
    Success = 0,
    Panic = 1,
}

#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegsKind {
    FsBase = 1,
}

impl RegsKind {
    pub const fn from_usize(value: usize) -> Option<Self> {
        match value {
            value if value == Self::FsBase as usize => Some(Self::FsBase),
            _ => None,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union Regs {
    pub fs_base: usize,
}

pub struct SyscallRegs {
    pub n: usize,
    pub a0: usize,
    pub a1: usize,
    pub a2: usize,
    pub a3: usize,
    pub a4: usize,
    pub a5: usize,
}

#[repr(C)]
pub struct SyscallFrame {
    pub cookie: usize,
    pub rflags: usize,
    pub rip: usize,
}
