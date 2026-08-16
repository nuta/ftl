use core::cmp::min;
use core::fmt;

use ftl_types::error::ErrorCode;
use ftl_types::vcpu::SyscallRegs;

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

pub fn sys_print(ctx: &SyscallRegs) -> Result<SyscallOutput, ErrorCode> {
    let mut addr = UAddr::new(ctx.a0);
    let mut len = ctx.a1;

    let mut buf = [0; 512];
    while len > 0 {
        let copy_len = min(len, buf.len());
        let slice = &mut buf[..copy_len];
        USlice::new(addr, copy_len)?.read(slice)?;
        crate::arch::console_write(slice);

        // TODO: Handle this in USlice (USliceReader?)
        addr = addr.add(copy_len).ok_or(ErrorCode::OUT_OF_BOUNDS)?;
        len -= copy_len;
    }

    Ok(SyscallOutput::Done(0))
}
