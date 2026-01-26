use crate::thread::return_to_user;

pub extern "C" fn syscall_handler(a0: usize) -> ! {
    println!("Hello from thread {}", a0 as u8 as char);
    for i in 0..0x100000 {
        use core::arch::asm;
        unsafe { asm!("nop") }
    }

    return_to_user();
}
