use core::fmt;

use ftl_types::error::ErrorCode;
use ftl_types::syscall::Syscall;

use crate::arch::syscall2;

fn sys_print(buf: *const u8, len: usize) -> Result<(), ErrorCode> {
    let bytes = unsafe { core::slice::from_raw_parts(buf, len) };
    let _ = syscall2(Syscall::Print, bytes.as_ptr() as usize, bytes.len())?;
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
            "\x1b[33mWARN\x1b[0m {}",
            format_args!($($arg)+)
        );
    }};
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)+) => {{
        $crate::println!("\x1b[31mERROR\x1b[0m {}", format_args!($($arg)+));
    }};
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)+) => {{
        $crate::println!("{}", format_args!($($arg)+));
    }};
}

#[macro_export]
macro_rules! println {
    ($message:expr) => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        writeln!($crate::print::Printer, concat!(env!("FTL_LOG_PREFIX"), $message)).ok();
    }};
    ($format:expr, $($arg:tt)*) => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        writeln!($crate::print::Printer, concat!(env!("FTL_LOG_PREFIX"), $format), $($arg)*).ok();
    }};
}
