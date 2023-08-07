use crate::arch::hang;
use crate::cpuvar::cpuvar;
use crate::backtrace::backtrace;
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicU8, Ordering};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // In case it panics while handling a panic, this panic handler implements
    // some fallback logic to try to at least print the panic details.
    match cpuvar().panic_counter.fetch_add(1, Ordering::SeqCst) {
        0 => {
            // First panic: Try whatever we can do including complicated stuff
            // which may panic again.
            println!("kernel panic: {}", info);
            backtrace();
            hang();
        }
        1 => {
            // Double panics: paniked while handling a panic.
            println!("double kernel panic: {:?}", info);
            hang();
        }
        _ => {
            // Triple panics: println! seems to be broken. Spin forever.
            hang();
        }
    }
}
