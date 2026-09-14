#[macro_export]
macro_rules! info {
    ($io:expr, $($arg:tt)+) => {{
        $crate::println!($io, "[{:<10}] {}", env!("CARGO_PKG_NAME"), format_args!($($arg)+));
    }};
}

#[macro_export]
macro_rules! warn {
    ($io:expr, $($arg:tt)+) => {{
        $crate::println!(
            $io,
            "[{:<10}] \x1b[33mWARN\x1b[0m: {}",
            env!("CARGO_PKG_NAME"),
            format_args!($($arg)+)
        );
    }};
}

#[macro_export]
macro_rules! error {
    ($io:expr, $($arg:tt)+) => {{
        $crate::println!($io, "[{:<10}] \x1b[31mERROR\x1b[0m: {}", env!("CARGO_PKG_NAME"), format_args!($($arg)+));
    }};
}

#[macro_export]
macro_rules! trace {
    ($io:expr, $($arg:tt)+) => {{
        $crate::println!($io, "[{:<10}] {}", env!("CARGO_PKG_NAME"), format_args!($($arg)+));
    }};
}

#[macro_export]
macro_rules! println {
    ($io:expr) => {{
        $io.print(format_args!(""));
    }};
    ($io:expr, $($arg:tt)*) => {{
        $io.print(format_args!($($arg)*));
    }};
}
