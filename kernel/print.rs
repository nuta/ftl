//! Printing utilities.
use core::fmt;

use crate::arch;

/// A private struct internally used in print macros. Don't use this directly
/// and use the `print!` and `println!` macros instead.
pub struct PrinterInternal;

impl fmt::Write for PrinterInternal {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        arch::console_write(s.as_bytes());
        Ok(())
    }
}

/// Prints a kernel message without a trailing newline.
#[macro_export]
macro_rules! print {
    ($($args:tt)*) => {{
        #![allow(unused_imports)]
        use core::fmt::Write;
        let _ = write!($crate::print::PrinterInternal, $($args)*);
    }};
}

/// Prints a kernel message with a trailing newline.
#[macro_export]
macro_rules! println {
    // println!()
    () => {{
        $crate::print!("\n");
    }};
    // println!("Hello World!")
    ($str:literal) => {{
        $crate::print!(concat!($str, "\n"));
    }};
    // println!("Hello {}!", "World")
    ($fmt:literal, $($args:tt)*) => {{
        $crate::print!(concat!($fmt, "\n"), $($args)*);
    }};
}

/// Dumps the given value.
#[macro_export]
macro_rules! dump {
    ($val:expr) => {{
        let value = $val;
        $crate::print!(concat!("{}:{}: ", stringify!($val), " = {:#?}\n"), file!(), line!(), value);
    }};
}
