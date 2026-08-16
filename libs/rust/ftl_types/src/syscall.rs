const SYS_NUMBER_BASE: usize = usize::MAX - 0xfff;

#[repr(usize)]
pub enum Syscall {
    Print = SYS_NUMBER_BASE + 1,
    VCpuExit = SYS_NUMBER_BASE + 2,
}
