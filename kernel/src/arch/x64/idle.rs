use core::arch::asm;

pub fn idle() -> ! {
    trace!("idle");
    loop {
        unsafe {
            asm!("sti", "hlt", "cli", options(nomem, nostack));
        }
    }
}
