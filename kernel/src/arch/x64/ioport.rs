use core::arch::asm;

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
