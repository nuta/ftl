#![no_std]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
// #[naked]
pub extern "C" fn start() {
    loop {}
}

