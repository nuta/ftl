use core::arch::asm;
use core::mem::size_of;

const BACKTRACE_MAX_DEPTH: usize = 16;

extern "C" {
    static __kernel_start: u8;
}

pub fn backtrace<F>(mut callback: F)
where
    F: FnMut(usize),
{
    println!("backtrace");
    todo!();
}
