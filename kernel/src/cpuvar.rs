use core::cell::UnsafeCell;

use crate::thread::Thread;

pub struct CpuVar {
    // TODO: This is error prone. Fix this.
    pub current_thread: UnsafeCell<*const Thread>,
}
