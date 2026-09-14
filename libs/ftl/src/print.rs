use core::fmt;

use ftl_types::error::ErrorCode;
use ftl_types::syscall::Syscall;

use crate::arch::syscall2;

fn sys_print(buf: *const u8, len: usize) -> Result<(), ErrorCode> {
    let mut bytes = unsafe { core::slice::from_raw_parts(buf, len) };
    while !bytes.is_empty() {
        let written = syscall2(Syscall::Print, bytes.as_ptr() as usize, bytes.len())?;
        if written == 0 {
            break;
        }
        bytes = &bytes[written..];
    }
    Ok(())
}

pub struct Printer;

impl fmt::Write for Printer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let _ = sys_print(s.as_ptr(), s.len());
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
