use ftl_types::error::ErrorCode;
use ftl_types::syscall::Syscall;

pub fn unreachable() -> ! {
    panic!("unreachable");
}

pub fn syscall1(n: Syscall, a0: usize) -> Result<usize, ErrorCode> {
    panic!("syscall1(0x{:x}, {a0:x}) is not implemented", n as usize);
}

pub fn syscall2(n: Syscall, a0: usize, a1: usize) -> Result<usize, ErrorCode> {
    panic!(
        "syscall2(0x{:x}, {a0:x}, {a1:x}) is not implemented",
        n as usize
    );
}

pub fn syscall4(
    n: Syscall,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
) -> Result<usize, ErrorCode> {
    panic!(
        "syscall4(0x{:x}, {a0:x}, {a1:x}, {a2:x}, {a3:x}) is not implemented",
        n as usize
    );
}
