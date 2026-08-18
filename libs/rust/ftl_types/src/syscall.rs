const SYS_NUMBER_BASE: i32 = i32::MAX - 0xfff;

#[repr(i32)]
pub enum Syscall {
    Print = SYS_NUMBER_BASE + 1,
    ThreadExit = SYS_NUMBER_BASE + 2,
    VmoCreate = SYS_NUMBER_BASE + 3,
}
