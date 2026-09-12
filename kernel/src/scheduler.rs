use alloc::collections::vec_deque::VecDeque;

use ftl_types::error::ErrorCode;
use ftl_utils::spinlock::SpinLock;

use crate::arch;
use crate::shared_ref::SharedRef;
use crate::thread::Thread;

pub static SCHEDULER: Scheduler = Scheduler::new();

struct Mutable {
    run_queue: VecDeque<SharedRef<Thread>>,
    /// The number of threads in this system.
    num_threads: usize,
}

pub struct Scheduler {
    mutable: SpinLock<Mutable>,
}

impl Scheduler {
    const fn new() -> Self {
        Self {
            mutable: SpinLock::new(Mutable {
                run_queue: VecDeque::new(),
                num_threads: 0,
            }),
        }
    }

    pub fn reserve_capacity(&self) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();

        mutable.num_threads += 1;
        if mutable.run_queue.capacity() < mutable.num_threads {
            // The runqueue is not large enough, allocate additional capacity.
            let additional = mutable.num_threads - mutable.run_queue.len();
            if mutable.run_queue.try_reserve(additional).is_err() {
                mutable.num_threads -= 1;
                return Err(ErrorCode::OutOfMemory);
            }
        }

        Ok(())
    }

    pub fn release_capacity(&self) {
        let mut mutable = self.mutable.lock();
        debug_assert!(mutable.num_threads > 0);
        mutable.num_threads -= 1;
    }

    /// Picks the next thread to run.
    pub fn pop(&self) -> Option<SharedRef<Thread>> {
        self.mutable.lock().run_queue.pop_front()
    }

    /// Pushes a runnable thread to the runqueue.
    pub fn push_back(&self, thread: SharedRef<Thread>) {
        let mut m = self.mutable.lock();
        debug_assert!(m.run_queue.len() < m.num_threads);
        m.run_queue.push_back(thread);
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

    if let Some(current) = current.thread()
        && current.is_runnable()
    {
        // The current thread is runnable. Push it back to the scheduler.
        SCHEDULER.push_back(current);
    }

    let next = loop {
        let Some(thread) = SCHEDULER.pop() else {
            // Clear the current thread. Otherwise, the interrupt handler would
            // overwrite the user's system call context (registers) with the idle
            // thread's context.
            current.clear();

            // No threads to run. Enter the idle loop.
            arch::idle();
            continue;
        };

        // Try resuming the thread if it is blocked.
        thread.try_wake();

        // The thread can be blocked while in the runqueue. Make sure it
        // is still runnable.
        if thread.is_runnable() {
            break thread;
        }
    };

    // Switch to the new thread.
    current.enter(next);
}
