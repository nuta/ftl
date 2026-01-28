use core::arch::asm;

use ftl_types::error::ErrorCode;

use crate::isolation::INKERNEL_ISOLATION;
use crate::shared_ref::SharedRef;
use crate::thread::Thread;

pub(super) unsafe fn out8(port: u16, value: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") value);
    };
}

pub(super) unsafe fn out16(port: u16, value: u16) {
    unsafe {
        asm!("out dx, ax", in("dx") port, in("ax") value);
    };
}

pub(super) unsafe fn out32(port: u16, value: u32) {
    unsafe {
        asm!("out dx, eax", in("dx") port, in("eax") value);
    };
}

pub(super) unsafe fn in8(port: u16) -> u8 {
    let value: u8;

    unsafe {
        asm!("in al, dx", in("dx") port, out("al") value);
    };

    value
}

pub(super) unsafe fn in16(port: u16) -> u16 {
    let value: u16;

    unsafe {
        asm!("in ax, dx", in("dx") port, out("ax") value);
    };

    value
}

pub(super) unsafe fn in32(port: u16) -> u32 {
    let value: u32;

    unsafe {
        asm!("in eax, dx", in("dx") port, out("eax") value);
    };

    value
}

pub fn sys_x64_iopl(thread: &SharedRef<Thread>, a0: usize) -> Result<usize, ErrorCode> {
    let _enable = a0 != 0;

    let isolation = thread.process().isolation();
    if SharedRef::ptr_eq(isolation, &INKERNEL_ISOLATION) {
        // The thread is running in kernel mode. No need to change the IOPL.
        Ok(0)
    } else {
        // The thread is running in user mode or something else. I guess we
        // won't need this. IOPL also enables STI/CLI instructions, which is
        // super dangerous.
        Err(ErrorCode::Unsupported)
    }
}
