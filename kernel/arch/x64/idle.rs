use core::arch::asm;

pub fn idle() -> ! {
    warn!("idle");
    loop {
        unsafe {
            asm!("sti; hlt");
        }
    }
}
