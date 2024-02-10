use core::fmt;

pub struct Printer;

impl core::fmt::Write for Printer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        crate::arch::console_write(s.as_bytes());
        Ok(())
    }
}

/// Prints a string.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        #![allow(unused_imports)]
        use core::fmt::Write;
        write!($crate::print::Printer, "{}", format_args!($($arg)*)).ok();
    }};
}

/// Prints a string and a newline.
#[macro_export]
macro_rules! println {
    () => {{
        $crate::print!(
            "\n"
        );
    }};
    ($fmt:expr) => {{
        crate::print!(
            concat!($fmt, "\n")
        );
    }};
    ($fmt:expr, $($arg:tt)*) => {{
        $crate::print!(
            concat!($fmt, "\n"),
            $($arg)*
        );
    }};
}

#[repr(transparent)]
pub struct ByteSize(usize);

impl ByteSize {
    pub const fn new(value: usize) -> ByteSize {
        ByteSize(value)
    }
}

impl fmt::Display for ByteSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let units = &["B", "KiB", "MiB", "GiB", "TiB"];
        let mut value = self.0;
        let mut i = 0;
        let mut unit = units[0];
        while value >= 1024 && i + 1 < units.len() {
            value /= 1024;
            unit = units[i + 1];
            i += 1;
        }

        write!(f, "{}{}", value, unit)
    }
}
