//! Printing utilities.
use crate::sbi;
use core::fmt;

/// A private struct internally used in print macros. Don't use this!
pub struct PrinterInternal;

impl fmt::Write for PrinterInternal {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.bytes() {
            unsafe {
                // Ignore errors. We can't do anything if something goes wrong
                // anyway.
                let _ = sbi::console_putchar(c as u8);
            }
        }
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
            ""
        );
    }};
    // println!("Hello World!")
    ($fmt:expr) => {{
        $crate::print!(
            $fmt
        );
    }};
    // println!("Hello {}!", "World")
    ($fmt:expr, $($arg:tt)*) => {{
        $crate::print!(
            concat!( $fmt, "\n"),
            $($arg)*
        );
    }};
}
