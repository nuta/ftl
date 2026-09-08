use core::mem::MaybeUninit;

use ftl_types::syscall::Syscall;
pub use ftl_types::time::MonoTime;

use crate::arch::syscall1;

pub fn now() -> MonoTime {
    let mut now = MaybeUninit::uninit();
    syscall1(Syscall::MonoTimeRead, now.as_mut_ptr() as usize).unwrap();
    // SAFETY: We've checked it did not return an error.
    unsafe { now.assume_init() }
}
