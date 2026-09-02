use core::arch::asm;

use ftl_types::error::ErrorCode;

use crate::address::UAddr;

pub unsafe fn usercopy_read(src: UAddr, dst: *mut u8, len: usize) -> Result<(), ErrorCode> {
    let retval: usize;
    unsafe {
        asm!(
            // RAX remains zero if rep movsb goes well. If it causes a page
            // fault, the interrupt handler jumps back to usercopy0_recover,
            // with RAX == 1.
            "xor eax, eax",
            ".global usercopy0; .set usercopy0, 2f; 2:",
            "rep movsb",
            ".global usercopy0_recover; .set usercopy0_recover, 3f; 3:",
            inout("rsi") src.as_usize() => _,
            inout("rdi") dst => _,
            inout("rcx") len => _,
            lateout("rax") retval,
            options(nostack),
        );
    }

    if retval != 0 {
        return Err(ErrorCode::PageFault);
    }

    Ok(())
}

pub unsafe fn usercopy_write(src: *const u8, dst: UAddr, len: usize) -> Result<(), ErrorCode> {
    let retval: usize;
    unsafe {
        asm!(
            "xor eax, eax",
            ".global usercopy1; .set usercopy1, 2f; 2:",
            "rep movsb",
            ".global usercopy1_recover; .set usercopy1_recover, 3f; 3:",
            inout("rsi") src => _,
            inout("rdi") dst.as_usize() => _,
            inout("rcx") len => _,
            lateout("rax") retval,
            options(nostack),
        );
    }

    if retval != 0 {
        return Err(ErrorCode::PageFault);
    }

    Ok(())
}
