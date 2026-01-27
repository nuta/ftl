#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ErrorCode {
    OutOfMemory,
    OutOfBounds,
    UnknownSyscall,
}
