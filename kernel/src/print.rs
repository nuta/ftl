use core::fmt;

use crate::arch;
pub struct Printer;

impl fmt::Write for Printer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        arch::console_write(s.as_bytes());
        Ok(())
    }
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)+) => {{
        $crate::println!("[kernel    ] INFO  {}", format_args!($($arg)+));
    }};
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)+) => {{
        $crate::println!(
            "[kernel    ] \x1b[33mWARN\x1b[0m  {}",
            format_args!($($arg)+)
        );
    }};
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)+) => {{
        $crate::println!("[kernel    ] \x1b[31mERROR\x1b[0m  {}", format_args!($($arg)+));
    }};
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)+) => {{
        $crate::println!("[kernel    ] {}", format_args!($($arg)+));
    }};
}

#[macro_export]
macro_rules! println {
    () => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        writeln!($crate::print::Printer).ok();
    }};
    ($($arg:tt)*) => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        writeln!($crate::print::Printer, $($arg)*).ok();
    }};
}
