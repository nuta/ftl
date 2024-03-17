use core::panic::PanicInfo;
use core::sync::atomic::AtomicU8;
use core::sync::atomic::Ordering;

use crate::arch::hang;

static PANIC_COUNTER: AtomicU8 = AtomicU8::new(0);

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // In case it panics while handling a panic, this panic handler implements
    // some fallback logic to try to at least print the panic details.
    match PANIC_COUNTER.fetch_add(1, Ordering::SeqCst) {
        0 => {
            // First panic: Try whatever we can do including complicated stuff
            // which may panic again.
            println!("kernel panic: {}", info);
            hang();
        }
        1 => {
            // Double panics: paniked while handling a panic. Keep it simple.
            println!("double kernel panic: {:?}", info);
            hang();
        }
        _ => {
            // Triple panics: println! seems to be broken. Spin forever.
            hang();
        }
    }
}
