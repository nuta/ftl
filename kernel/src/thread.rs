use crate::arch;
use crate::scheduler::SCHEDULER;

pub struct Thread {
    pub arch: arch::Thread,
}

impl Thread {
    pub fn new(entry: usize, sp: usize, arg: usize) -> Self {
        Self {
            arch: arch::Thread::new(entry, sp, arg),
        }
    }
}

fn schedule() -> Option<*const arch::Thread> {
    let thread = SCHEDULER.pop()?;

    let arch_ptr = &raw const thread.arch;
    core::mem::forget(thread);

    Some(arch_ptr)
}

/// Jumps to a thread.
///
/// In other words, it leaves the kernel. The kernel will be resumed when
/// an exception or interrupt occurs.
///
/// Unlike traditional operating systems, this function never returns due to
/// its single stack design.
pub fn return_to_user() -> ! {
    let Some(thread) = schedule() else {
        // No threads to run. Enter the idle loop.
        arch::idle();
    };

    arch::thread_switch(thread);
}
