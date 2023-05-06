//! Printing utilities.
use core::fmt;

use crate::arch;

/// A private struct internally used in print macros. Don't use this!
pub struct PrinterInternal;

impl fmt::Write for PrinterInternal {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        arch::console_write(s.as_bytes());
        Ok(())
    }
}

/// Prints a string.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        #![allow(unused_imports)]
        use core::fmt::Write;
        write!($crate::print::PrinterInternal, "{}", format_args!($($arg)*)).ok();
    }};
}

/// Prints a string and a newline.
#[macro_export]
macro_rules! println {
    // println!()
    () => {{
        $crate::print!(
            "\n"
        );
    }};
    // println!("Hello World!")
    ($fmt:expr) => {{
        $crate::print!(
            concat!($fmt, "\n"),
        );
    }};
    // println!("Hello {}!", "World")
    ($fmt:expr, $($arg:tt)*) => {{
        $crate::print!(
            concat!($fmt, "\n"),
            $($arg)*
        );
    }};
}
