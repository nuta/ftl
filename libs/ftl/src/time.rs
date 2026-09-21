use core::mem::MaybeUninit;

use ftl_types::syscall::Syscall;
pub use ftl_types::time::MonoTime;
pub use ftl_types::time::WallTime;

use crate::arch::syscall1;

pub trait MonoTimeExt: Sized {
    fn now() -> Self;
}

pub trait WallTimeExt: Sized {
    fn now() -> Self;
}

impl MonoTimeExt for MonoTime {
    fn now() -> Self {
        let mut now = MaybeUninit::uninit();
        syscall1(Syscall::MonoTimeRead, now.as_mut_ptr() as usize).unwrap();
        // SAFETY: We've checked it did not return an error.
        unsafe { now.assume_init() }
    }
}

impl WallTimeExt for WallTime {
    fn now() -> Self {
        let mut now = MaybeUninit::uninit();
        syscall1(Syscall::WallTimeRead, now.as_mut_ptr() as usize).unwrap();
        // SAFETY: We've checked it did not return an error.
        unsafe { now.assume_init() }
    }
}
