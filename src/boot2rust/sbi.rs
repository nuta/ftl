//! SBI (Supervisor Binary Interface) implementation.
//!
//! See <https://github.com/riscv-non-isa/riscv-sbi-doc/blob/master/riscv-sbi.pdf>
//! for the specification.
use core::result::Result;

/// SBI extension IDs.
#[repr(isize)]
enum SbiExtId {
    ConsolePutChar = 0x01,
    Shutdown = 0x08,
}

// SBI error codes.
pub type SbiErrorCode = isize;
#[allow(unused)]
pub const SBI_SUCCESS: isize = 0;
#[allow(unused)]
pub const SBI_ERR_FAILED: isize = -1;
#[allow(unused)]
pub const SBI_ERR_NOT_SUPPORTED: isize = -2;
#[allow(unused)]
pub const SBI_ERR_INVALID_PARAM: isize = -3;
#[allow(unused)]
pub const SBI_ERR_DENIED: isize = -4;
#[allow(unused)]
pub const SBI_ERR_INVALID_ADDRESS: isize = -5;
#[allow(unused)]
pub const SBI_ERR_ALREADY_AVAILABLE: isize = -6;
#[allow(unused)]
pub const SBI_ERR_ALREADY_STARTED: isize = -7;
#[allow(unused)]
pub const SBI_ERR_ALREADY_STOPPED: isize = -8;

/// Calls a SBI function. See "Chapter 3. Binary Encoding" in the SBI specification.
unsafe fn sbi_call(
    extid: SbiExtId,
    a0: isize,
    a1: isize,
    a2: isize,
    a3: isize,
    a4: isize,
    a5: isize,
) -> Result<isize, SbiErrorCode> {
    core::arch::asm!(
        "ecall",
        inout("a0") a0 => _,
        inout("a1") a1 => _,
        in("a2") a2,
        in("a3") a3,
        in("a4") a4,
        in("a5") a5,
        in("a6") 0, // Function ID
        in("a7") extid as isize,
    );

    if a0 == SBI_SUCCESS {
        Ok(a1)
    } else {
        Err(a0)
    }
}

/// Writes a character to the debug console. Deprecated.
pub unsafe fn console_putchar(c: u8) -> Result<(), SbiErrorCode> {
    let _ = sbi_call(SbiExtId::ConsolePutChar, c as isize, 0, 0, 0, 0, 0)?;
    Ok(())
}

/// Puts all harts to shutdown state from the supervisor point of view. Never returns.
pub unsafe fn shutdown() -> ! {
    let _ = sbi_call(SbiExtId::Shutdown, 0, 0, 0, 0, 0, 0);
    unreachable!()
}
