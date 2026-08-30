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
    ThreadWriteRegs = SYSCALL_BASE + 9,
    ThreadCopyRegs = SYSCALL_BASE + 10,
    PollCreate = SYSCALL_BASE + 11,
    PollWait = SYSCALL_BASE + 12,
    PollNotify = SYSCALL_BASE + 13,
    NetAcquire = SYSCALL_BASE + 14,
    NetPeek = SYSCALL_BASE + 15,
    NetRecv = SYSCALL_BASE + 16,
    NetSend = SYSCALL_BASE + 17,
}
