use alloc::collections::VecDeque;

use crate::thread::Thread;

pub static GLOBAL_SCHEDULER: Scheduler = Scheduler::new();

pub struct Scheduler {
    runqueue: VecDeque<Thread>,
}

impl Scheduler {
    pub const fn new() -> Scheduler {
        Scheduler {
            runqueue: VecDeque::new(),
        }
    }

    pub fn yield_cpu(&self) {

    }
}

