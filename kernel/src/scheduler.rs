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
        let thread = runqueue.pop_front()?;
        Some(thread)
    }

    /// Pushes a runnable thread to the runqueue.
    pub fn push(&self, thread: SharedRef<Thread>) {
        self.runqueue.lock().push_back(thread);
    }

    /// Pushes a runnable thread to the front of the runqueue, so that it willl
    /// be picked first.
    pub fn push_front(&self, thread: SharedRef<Thread>) {
        self.runqueue.lock().push_front(thread);
    }
}
