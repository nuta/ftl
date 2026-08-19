const SYS_NUMBER_BASE: i32 = i32::MAX - 0xfff;

#[repr(i32)]
pub enum Syscall {
    Print = SYS_NUMBER_BASE + 1,
    ThreadExit = SYS_NUMBER_BASE + 2,
    VmoCreate = SYS_NUMBER_BASE + 3,
    VmoWrite = SYS_NUMBER_BASE + 4,
    VmSpaceClone = SYS_NUMBER_BASE + 5,
    VmSpaceMap = SYS_NUMBER_BASE + 6,
    ThreadCreate = SYS_NUMBER_BASE + 7,
    ThreadStart = SYS_NUMBER_BASE + 8,
}
