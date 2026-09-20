use core::fmt;

pub struct Printer;

impl Printer {
    fn do_write(&mut self, mut bytes: &[u8]) {
        while !bytes.is_empty() {
            match crate::console::write(bytes) {
                Ok(0) => {
                    // The device is full.
                    break;
                }
                Ok(written) => {
                    bytes = &bytes[written..];
                }
                Err(_) => {
                    // TODO: Handle error.
                    break;
                }
            }
        }
    }
}

impl fmt::Write for Printer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.do_write(s.as_bytes());
        Ok(())
    }
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)+) => {{
        $crate::println!("{}", format_args!($($arg)+));
    }};
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)+) => {{
        $crate::println!(
            "\x1b[33mWARN\x1b[0m: {}",
            format_args!($($arg)+)
        );
    }};
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)+) => {{
        $crate::println!("\x1b[31mERROR\x1b[0m: {}", format_args!($($arg)+));
    }};
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)+) => {{
        #[cfg(debug_assertions)]
        {
            $crate::println!("{}", format_args!($($arg)+));
        }
    }};
}

#[macro_export]
macro_rules! println {
    ($message:expr) => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        writeln!($crate::print::Printer, "[{:<10}] {}", env!("CARGO_PKG_NAME"), $message).ok();
    }};
    ($format:expr, $($arg:tt)*) => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        writeln!($crate::print::Printer, concat!("[{:<10}] ", $format), env!("CARGO_PKG_NAME"), $($arg)*).ok();
    }};
}
