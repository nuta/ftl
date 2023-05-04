#![no_std]

core::arch::global_asm!(include_str!("boot.S"));

mod sbi;

#[no_mangle]
pub fn rust_entry() {
    for c in b"\n\n\x1b[1;35mHello from Rust World!\x1b[0m\n\n\n" {
        unsafe {
            let _ = sbi::console_putchar(*c);
        }
    }

    unsafe {
        sbi::shutdown();
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
