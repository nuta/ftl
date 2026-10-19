use alloc::collections::vec_deque::VecDeque;

use ftl_types::error::ErrorCode;
use ftl_types::time::MonoTime;
use ftl_utils::spinlock::SpinLock;

use crate::arch;
use crate::quota::CpuTimeQuota;
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

    pub fn cancel(&self, thread: &SharedRef<Thread>) {
        self.mutable
            .lock()
            .run_queue
            .retain(|queued| !SharedRef::eq(queued, thread));
    }

    /// Picks the next thread to run.
    pub fn pop(&self, now: MonoTime) -> Option<SharedRef<Thread>> {
        let mut m = self.mutable.lock();

        // Iterate over the queue and find the first truely runnable one.
        for i in 0..m.run_queue.len() {
            let thread = &m.run_queue[i];
            if !thread.quota().is_exhausted::<CpuTimeQuota>(now) {
                return m.run_queue.remove(i);
            }
        }

        None
    }

    /// Pushes a runnable thread to the runqueue.
    pub fn push_back(&self, thread: SharedRef<Thread>) {
        let mut m = self.mutable.lock();
        debug_assert!(m.run_queue.len() < m.num_threads);
        m.run_queue.push_back(thread);
    }
}

/// Why context switching (`return_to_user`) happens.
pub enum Resched {
    /// The first context switch in this CPU.
    Init,
    /// The current thread can continue running, but the kernel decided to
    /// preempt and schedule a new thread to run.
    Preempt,
    /// The current thread is now blocked.
    Block,
}

/// Schedules a new thread to run, leave the kernel, and jumps to it.
///
/// The kernel will be resumed when an exception or interrupt occurs.
///
/// Unlike traditional operating systems, this function never returns because of
/// the single kernel stack design.
pub fn return_to_user(resched: Resched) -> ! {
    let cpuvar = arch::get_cpuvar();
    let current = &cpuvar.current_thread;

    let mut now = arch::monotime_read();
    if !matches!(resched, Resched::Init) {
        let thread = current.thread();
        let switched_at = cpuvar.switched_at.get();

        // SAFETY: `switched_at` was set by this CPU before entering the
        //         thread.
        let elapsed = now.duration_since(switched_at).unwrap();

        // Charge the CPU time.
        thread.quota().charge_periodic::<CpuTimeQuota>(now, elapsed);

        // Requeue the current thread to the runqueue if it is runnable.
        //
        // The thread might have been terminated by a different CPU. Check
        // the latest state.
        if matches!(resched, Resched::Preempt) && thread.is_runnable() {
            SCHEDULER.push_back(thread);
        }
    }

    loop {
        // Find the next thread to switch to.
        let next = loop {
            let Some(thread) = SCHEDULER.pop(now) else {
                // No threads to run. Enter the idle loop.
                arch::vmspace_switch_to_kernel();
                arch::idle();
                now = arch::monotime_read();
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

        // Jump into the next thread. This function does not return on success.
        cpuvar.switched_at.set(now);
        let Err(err) = current.enter(next);

        // Failed to enter the thread due to its invalid state. Terminate it.
        trace!("failed to enter a thread, terminating it: {:?}", err);
        if let Err(err) = current.thread().exit() {
            trace!("failed to exit the thread: {:?}", err);
        }
    }
}
