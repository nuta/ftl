use core::arch::asm;

pub fn idle() -> ! {
    println!("entering idle loop");
    loop {
        unsafe {
            asm!("swapgs; sti; hlt");
        }
    }
}

pub fn halt() -> ! {
    loop {
        unsafe {
            asm!("cli; hlt");
        }
    }
}
