#[macro_export]
macro_rules! warn {
    ($io:expr, $($arg:tt)+) => {{
        $crate::println!(
            $io,
            "[driver   ] \x1b[33mWARN\x1b[0m  {}",
            format_args!($($arg)+)
        );
    }};
}

#[macro_export]
macro_rules! error {
    ($io:expr, $($arg:tt)+) => {{
        $crate::println!($io, "[driver   ] \x1b[31mERROR\x1b[0m  {}", format_args!($($arg)+));
    }};
}

#[macro_export]
macro_rules! trace {
    ($io:expr, $($arg:tt)+) => {{
        $crate::println!($io, "[driver   ] {}", format_args!($($arg)+));
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
