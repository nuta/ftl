use core::arch::asm;

pub fn idle() -> ! {
    loop {
        unsafe {
            asm!("sti", "hlt", "cli", options(nomem, nostack));
            crate::scheduler::return_to_user();
        }
    }
}
