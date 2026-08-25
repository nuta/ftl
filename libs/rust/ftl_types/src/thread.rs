#[repr(usize)]
pub enum ExitReason {
    Success = 0,
    Panic = 1,
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
