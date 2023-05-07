//! SBI (Supervisor Binary Interface) implementation.
//!
//! See <https://github.com/riscv-non-isa/riscv-sbi-doc/blob/master/riscv-sbi.pdf>
//! for the specification.
#![allow(unused)]

use core::arch::asm;
use core::result::Result;

/// SBI extension IDs.
#[repr(usize)]
enum SbiExtId {
    ConsolePutChar = 0x01,
    Shutdown = 0x08,
}

// SBI error codes.
pub type SbiErrorCode = isize;
pub const SBI_SUCCESS: isize = 0;
pub const SBI_ERR_FAILED: isize = -1;
pub const SBI_ERR_NOT_SUPPORTED: isize = -2;
pub const SBI_ERR_INVALID_PARAM: isize = -3;
pub const SBI_ERR_DENIED: isize = -4;
pub const SBI_ERR_INVALID_ADDRESS: isize = -5;
pub const SBI_ERR_ALREADY_AVAILABLE: isize = -6;
pub const SBI_ERR_ALREADY_STARTED: isize = -7;
pub const SBI_ERR_ALREADY_STOPPED: isize = -8;

// PMU is only supported partially in QEMU.
// WIP: https://gist.github.com/nuta/84eff47225fa2e6a7337034e920b5c1e

/// Calls a SBI function. See "Chapter 3. Binary Encoding" in the SBI specification.
unsafe fn sbi_call(
    extid: SbiExtId,
    funcid: usize,
    mut a0: usize,
    mut a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
) -> Result<usize, SbiErrorCode> {
    asm!(
        "ecall",
        inout("a0") a0 => a0,
        inout("a1") a1 => a1,
        in("a2") a2,
        in("a3") a3,
        in("a4") a4,
        in("a5") a5,
        in("a6") funcid,
        in("a7") extid as usize,
    );

    let err = a0 as isize;
    if err == SBI_SUCCESS {
        Ok(a1)
    } else {
        Err(err)
    }
}

/// Writes a character to the debug console. Deprecated.
pub fn console_putchar(c: u8) -> Result<(), SbiErrorCode> {
    unsafe {
        let _ =
            sbi_call(SbiExtId::ConsolePutChar, 0, c as usize, 0, 0, 0, 0, 0)?;
    }
    Ok(())
}

/// Puts all harts to shutdown state from the supervisor point of view. Never returns.
pub fn shutdown() -> ! {
    unsafe {
        let _ = sbi_call(SbiExtId::Shutdown, 0, 0, 0, 0, 0, 0, 0);
    }
    unreachable!()
}
