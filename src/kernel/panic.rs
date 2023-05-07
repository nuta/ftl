use crate::arch::hang;
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicU8, Ordering};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // In case it panics while handling a panic, this panic handler implements
    // some fallback logic to try to at least print the panic details.
    static PANIC_COUNTER: AtomicU8 = AtomicU8::new(0); // TODO: Use cpu-local storage.
    match PANIC_COUNTER.fetch_add(1, Ordering::SeqCst) {
        0 => {
            // Continue panic handling.
        }
        1 => {
            // Paniked while handling a panic: print details and abort.
            println!("kernel panic: {:?}", info);
            hang();
        }
        _ => {
            // Too nested panics: println! seems to be broken. Spin forever.
            hang();
        }
    }

    // This is the first panic. Try whatever we can do including complicated stuff
    // which may panic again.
    println!("kernel panic: {}", info);
    hang();
}
