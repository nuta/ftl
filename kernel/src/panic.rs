use core::panic::PanicInfo;

use crate::arch;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    trace!("\nkernel panic: {info}");
    arch::halt();
}
