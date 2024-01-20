use core::arch::asm;

mod sbi;

pub fn idle() {
    unsafe {
        asm!("wfi");
    }
}

pub fn hang() -> ! {
    loop {
        unsafe {
            asm!("wfi");
        }
    }
}

pub fn console_write(bytes: &[u8]) {
    for byte in bytes {
        sbi::console_putchar(*byte);
    }
}
