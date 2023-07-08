#![no_std]
#![feature(naked_functions)]

mod arch;

use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    loop {}
}
