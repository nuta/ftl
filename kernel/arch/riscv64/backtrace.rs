use core::{arch::asm, mem::size_of};

use crate::{address::VAddr, backtrace::CapturedBacktrace};

const BACKTRACE_MAX: usize = 16;

#[repr(C, packed)]
pub struct StackFrame {
    fp: u64,
    ra: u64,
}

pub fn backtrace<F>(mut callback: F)
where
    F: FnMut(usize, VAddr),
{
    let mut fp: u64;
    let mut ra: u64;
    unsafe {
        asm!(r#"
                mv {}, fp
                mv {}, ra
            "#,
            out(reg) fp,
            out(reg) ra,
        );
    }

    if !super::is_valid_vaddr(ra as usize) {
        return;
    }

    for i in 0..BACKTRACE_MAX {
        if !super::is_valid_vaddr(ra as usize) {
            break;
        }

        callback(i, VAddr::new(ra as usize));

        let frame = fp as *const StackFrame;
        if frame.is_null() || !super::is_valid_vaddr(frame as usize) {
            break;
        }

        unsafe {
            fp = (*frame).fp.saturating_sub(size_of::<StackFrame>() as u64);
            ra = (*frame).ra;
        }
    }
}
