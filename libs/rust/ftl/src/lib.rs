#![no_std]

use core::fmt;

mod arch;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    println!("panic: {}", _info);
    loop {}
}

pub struct Printer;

impl fmt::Write for Printer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        //
        Ok(())
    }
}

#[macro_export]
macro_rules! println {
    () => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        writeln!($crate::Printer).ok();
    }};
    ($fmt:expr) => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        writeln!($crate::Printer, $fmt).ok();
    }};
    ($fmt:expr, $($arg:tt)*) => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        writeln!($crate::Printer, $fmt, $($arg)*).ok();
    }};
}
