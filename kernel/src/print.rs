use core::cmp::min;
use core::fmt;

use ftl_types::error::ErrorCode;

use crate::arch;
use crate::isolation::UserPtr;
use crate::isolation::UserSlice;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallResult;
use crate::thread::Thread;

pub struct Printer;

impl fmt::Write for Printer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        arch::console_write(s.as_bytes());
        Ok(())
    }
}

pub fn sys_console_write(
    current: &SharedRef<Thread>,
    buf_ptr: usize,
    len: usize,
) -> Result<SyscallResult, ErrorCode> {
    let slice = UserSlice::new(UserPtr::new(buf_ptr), len)?;
    let isolation = current.process().isolation();

    let mut offset = 0;
    while offset < slice.len() {
        let mut tmp = [0u8; 256];
        let copy_len = min(slice.len() - offset, tmp.len());

        // Copy the string from the user space.
        let subslice = slice.subslice(offset, copy_len)?;
        isolation.read_bytes(&subslice, &mut tmp[..copy_len])?;

        arch::console_write(&tmp[..copy_len]);
        offset += copy_len;
    }

    Ok(SyscallResult::Return(0))
}

#[macro_export]
macro_rules! println {
    () => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        writeln!($crate::print::Printer).ok();
    }};
    ($fmt:expr) => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        writeln!($crate::print::Printer, $fmt).ok();
    }};
    ($fmt:expr, $($arg:tt)*) => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        writeln!($crate::print::Printer, $fmt, $($arg)*).ok();
    }};
}
