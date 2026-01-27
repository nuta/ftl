#![no_std]

use core::fmt;

use ftl_types::syscall::SYS_CONSOLE_WRITE;

use crate::arch::get_start_info;

mod arch;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    println!("panic: {}", _info);
    loop {}
}

pub struct Printer;

impl fmt::Write for Printer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        (get_start_info().syscall)(
            s.as_bytes().as_ptr() as usize,
            s.len(),
            0,
            0,
            0,
            SYS_CONSOLE_WRITE,
        );
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
