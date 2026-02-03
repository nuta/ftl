use alloc::string::String;
use alloc::string::ToString;
use core::fmt;

use crate::syscall::sys_console_write;

pub struct Printer;

static BUFFER: spin::Mutex<String> = spin::Mutex::new(String::new());

impl fmt::Write for Printer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut buffer = BUFFER.lock();
        buffer.push_str(s);
        while let Some(index) = buffer.find('\n') {
            sys_console_write(buffer[..(index + 1)].as_bytes());
            *buffer = buffer[index + 1..].to_string();
        }

        Ok(())
    }
}

#[macro_export]
macro_rules! println {
    () => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        writeln!($crate::print::Printer).ok();
    }};
    ($fmt:expr) => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        writeln!($crate::print::Printer, $fmt).ok();
    }};
    ($fmt:expr, $($arg:tt)*) => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        writeln!($crate::print::Printer, $fmt, $($arg)*).ok();
    }};
}
