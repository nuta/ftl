enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

#[macro_export]
macro_rules! debug {
    ($message:literal) => {{
        $crate::arch::log!($crate::logger::Level::Debug, $message);
    }};
    ($format:literal, $($arg:tt)*) => {{
        $crate::arch::log!($crate::logger::Level::Debug, $format, $($arg)*);
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
