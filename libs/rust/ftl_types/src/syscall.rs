pub const SYSCALL_BASE: usize = usize::MAX - 0x1000;

#[repr(usize)]
pub enum Syscall {
    Print = SYSCALL_BASE + 1,
    ThreadExit = SYSCALL_BASE + 2,
    VmoCreate = SYSCALL_BASE + 3,
    VmoWrite = SYSCALL_BASE + 4,
    VmSpaceClone = SYSCALL_BASE + 5,
    VmSpaceMap = SYSCALL_BASE + 6,
    ThreadCreate = SYSCALL_BASE + 7,
    ThreadStart = SYSCALL_BASE + 8,
}
