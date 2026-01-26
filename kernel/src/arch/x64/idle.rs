use core::arch::asm;

pub fn idle() -> ! {
    println!("entering idle loop");
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}
