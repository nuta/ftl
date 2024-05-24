use alloc::collections::VecDeque;
use alloc::sync::Arc;

use crate::arch::cpuvar;
use crate::spinlock::SpinLock;
use crate::thread::Thread;

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
        let next = {
            let cpuvar = cpuvar();
            let mut current_thread = cpuvar.current_thread.borrow_mut();
            let mut runqueue = self.runqueue.lock();

            if current_thread.is_runnable() {
                runqueue.push_back(current_thread.clone());
            }

            // Get the next thread to run. If the runqueue is empty, run the
            // idle thread.
            let next = runqueue
                .pop_front()
                .unwrap_or_else(|| cpuvar.idle_thread.clone());

            *current_thread = next.clone();
            next
        };

        next.switch_to_this();
    }
}
