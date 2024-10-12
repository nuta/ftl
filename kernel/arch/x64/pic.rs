use core::arch::asm;

fn out8(port: u16, value: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") value, options(nostack));
    }
}

pub fn init() {
    // Disables PIC. We use IO APIC instead.
    unsafe {
        out8(0xa1, 0xff);
        out8(0x21, 0xff);
        out8(0x20, 0x11);
        out8(0xa0, 0x11);
        out8(0x21, 0x20);
        out8(0xa1, 0x28);
        out8(0x21, 0x04);
        out8(0xa1, 0x02);
        out8(0x21, 0x01);
        out8(0xa1, 0x01);
        out8(0xa1, 0xff);
        out8(0x21, 0xff);
    }
}
