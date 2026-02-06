use core::arch::asm;

pub fn idle() -> ! {
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
