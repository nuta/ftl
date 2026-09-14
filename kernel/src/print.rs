use core::cmp::min;
use core::fmt;

use ftl_types::error::ErrorCode;
use ftl_types::thread::SyscallRegs;

use crate::address::UAddr;
use crate::address::USlice;
use crate::arch;
use crate::syscall::SyscallOutput;
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
        $crate::println!("[kernel    ] {}", format_args!($($arg)+));
    }};
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)+) => {{
        $crate::println!(
            "[kernel    ] \x1b[33mWARN\x1b[0m: {}",
            format_args!($($arg)+)
        );
    }};
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)+) => {{
        $crate::println!("[kernel    ] \x1b[31mERROR\x1b[0m: {}", format_args!($($arg)+));
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

const MAX_PRINT_LEN: usize = 512;

pub fn sys_print(ctx: &SyscallRegs) -> Result<SyscallOutput, ErrorCode> {
    let len = min(ctx.a1, MAX_PRINT_LEN);
    if len == 0 {
        return Ok(SyscallOutput::Done(0));
    }

    let mut buf = [0; MAX_PRINT_LEN];
    let slice = &mut buf[..len];
    USlice::new(UAddr::new(ctx.a0), len)?.read_bytes(slice)?;
    crate::arch::console_write(slice);

    Ok(SyscallOutput::Done(len))
}
