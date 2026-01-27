use alloc::collections::vec_deque::VecDeque;

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
        // FIXME: do not add back
        let thread = runqueue.pop_front()?;
        runqueue.push_back(thread.clone());
        Some(thread)
    }

    /// Pushes a runnable thread to the runqueue.
    pub fn push(&self, thread: SharedRef<Thread>) {
        self.runqueue.lock().push_back(thread);
    }
}
