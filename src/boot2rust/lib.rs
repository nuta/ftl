#![no_std]

core::arch::global_asm!(include_str!("boot.S"));

#[no_mangle]
pub fn rust_entry() {
    let thr = 0x10000000 as *mut u8;
    for c in b"Hello from Rust World!\n" {
        unsafe {
            core::ptr::write_volatile(thr, *c);
        }
    }

    let sifive_test = 0x100000 as *mut u32;
    unsafe {
        core::ptr::write_volatile(sifive_test, 0x5555);
    }

    loop {}
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
