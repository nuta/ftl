use core::panic::PanicInfo;

use crate::arch;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("\nkernel panic: {info}");
    arch::halt();
}
