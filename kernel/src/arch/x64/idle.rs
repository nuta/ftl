use core::arch::asm;

pub fn idle() {
    unsafe {
        asm!("sti", "hlt", "cli", options(nomem, nostack));
    }
}
