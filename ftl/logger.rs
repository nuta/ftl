use core::fmt;

enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

pub struct Printer;

impl fmt::Write for Printer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        use std::io::{stderr, Write};
        stderr().write_all(s.as_bytes()).unwrap();

        Ok(())
    }
}

#[macro_export]
macro_rules! log {
    ($level:expr, $message:literal) => {{
        use core::fmt::Write;
        let _ = write!($crate::logger::Printer, "{}\n", $message);
    }};
    ($level:expr, $format:literal, $($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($crate::logger::Printer, concat!($format, "\n"), $($arg)*);
    }};
}

#[macro_export]
macro_rules! debug {
    ($message:literal) => {{
        $crate::log!($crate::logger::Level::Debug, $message);
    }};
    ($format:literal, $($arg:tt)*) => {{
        $crate::log!($crate::logger::Level::Debug, $format, $($arg)*);
    }};
}

#[macro_export]
macro_rules! info {
    ($message:literal) => {{
        $crate::log!($crate::logger::Level::Info, $message);
    }};
    ($format:literal, $($arg:tt)*) => {{
        $crate::log!($crate::logger::Level::Info, $format, $($arg)*);
    }};
}

#[macro_export]
macro_rules! warn {
    ($message:literal) => {{
        $crate::log!($crate::logger::Level::Warn, $message);
    }};
    ($format:literal, $($arg:tt)*) => {{
        $crate::log!($crate::logger::Level::Warn, $format, $($arg)*);
    }};
}

#[macro_export]
macro_rules! error {
    ($message:literal) => {{
        $crate::log!($crate::logger::Level::Error, $message);
    }};
    ($format:literal, $($arg:tt)*) => {{
        $crate::log!($crate::logger::Level::Error, $format, $($arg)*);
    }};
}

#[macro_export]
macro_rules! debug_warn {
    ($message:literal) => {{
        #[cfg(debug_assertions)]
        $crate::log!($crate::logger::Level::Warn, $message);
    }};
    ($format:literal, $($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        $crate::log!($crate::logger::Level::Warn, $format, $($arg)*);
    }};
}
