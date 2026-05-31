#![no_std]

#[inline(never)]
pub fn foo() {
    unsafe {
        core::arch::asm!("nop");
    }
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
