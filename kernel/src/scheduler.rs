use alloc::collections::vec_deque::VecDeque;

use crate::arch;
use crate::error::ErrorCode;
use crate::shared_ref::SharedRef;
use crate::spinlock::SpinLock;
use crate::thread::Thread;

pub static SCHEDULER: Scheduler = Scheduler::new();

pub struct Scheduler {
    runqueue: SpinLock<VecDeque<SharedRef<Thread>>>,
}

impl Scheduler {
    const fn new() -> Self {
        Self {
            runqueue: SpinLock::new(VecDeque::new()),
        }
    }

    /// Picks the next thread to run.
    pub fn pop(&self) -> Option<SharedRef<Thread>> {
        let mut runqueue = self.runqueue.lock();
        let thread = runqueue.pop_front()?;
        Some(thread)
    }

    /// Pushes a runnable thread to the runqueue.
    pub fn push_back(&self, thread: SharedRef<Thread>) -> Result<(), ErrorCode> {
        let mut runqueue = self.runqueue.lock();
        if runqueue.try_reserve(1).is_err() {
            return Err(ErrorCode::OutOfMemory);
        }

        runqueue.push_back(thread);
        Ok(())
    }

    /// Pushes a runnable thread to the front of the runqueue, so that it willl
    /// be picked first.
    pub fn push_front(&self, thread: SharedRef<Thread>) -> Result<(), ErrorCode> {
        let mut runqueue = self.runqueue.lock();
        if runqueue.try_reserve(1).is_err() {
            return Err(ErrorCode::OutOfMemory);
        }

        runqueue.push_front(thread);
        Ok(())
    }
}

/// Schedules a new thread to run, leave the kernel, and jumps to it.
///
/// The kernel will be resumed when an exception or interrupt occurs.
///
/// Unlike traditional operating systems, this function never returns because of
/// the single kernel stack design.
pub fn return_to_user() -> ! {
    let cpuvar = arch::get_cpuvar();
    let current = &cpuvar.current_thread;

    if let Some(current) = current.thread() {
        if current.is_runnable() {
            // The current thread is runnable. Push it back to the scheduler.
            SCHEDULER
                .push_front(current)
                .expect("out of memory in runqueue"); // FIXME:
        }
    }

    let Some(thread) = SCHEDULER.pop() else {
        // Clear the current thread. Otherwise, the interrupt handler would
        // overwrite the user's system call context (registers) with the idle
        // thread's context.
        current.clear();

        // No threads to run. Enter the idle loop.
        arch::idle();
    };

    // Update the current thread.
    let arch_thread = current.update(thread);

    // Note: Drop references before calling this; this function never returns.
    arch::Thread::enter(arch_thread);
}
