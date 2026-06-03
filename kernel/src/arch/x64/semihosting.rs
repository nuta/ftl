use core::hint::spin_loop;

use super::ioport::out32;

/// QEMU's isa-debug-exit device.
const ISA_DEBUG_EXIT_PORT: u16 = 0x501;

#[allow(dead_code)]
pub fn semihosting_exit() -> ! {
    let value = 35; // Exit code will be (value << 1) | 1 = 71
    unsafe { out32(ISA_DEBUG_EXIT_PORT, value) };

    loop {
        spin_loop();
    }
}
