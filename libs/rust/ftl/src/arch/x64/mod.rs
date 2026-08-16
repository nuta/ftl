use core::arch::asm;

pub fn unreachable() -> ! {
    unsafe {
        asm!("ud2", options(noreturn));
    }
}
