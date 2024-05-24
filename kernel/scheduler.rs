use alloc::{collections::VecDeque, sync::Arc};

use crate::{arch::{self, cpuvar}, spinlock::SpinLock, thread::Thread};

pub static GLOBAL_SCHEDULER: Scheduler = Scheduler::new();

pub struct Scheduler {
    runqueue: SpinLock<VecDeque<Arc<Thread>>>,
}

impl Scheduler {
    pub const fn new() -> Scheduler {
        Scheduler {
            runqueue: SpinLock::new(VecDeque::new()),
        }
    }

    pub fn push(&self, new_thread: Arc<Thread>) {
        self.runqueue.lock().push_back(new_thread);
    }

    pub fn yield_cpu(&self) {
        let next = self.runqueue.lock().pop_front().unwrap_or_else(|| {
            cpuvar().idle_thread.clone()
        });

        *cpuvar().current_thread.borrow_mut() = next.clone();
        next.switch_to_this();
    }
}

