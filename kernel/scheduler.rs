use alloc::vec::Vec;

use crate::{ref_count::SharedRef, giant_lock::GiantLock, thread::Thread};

pub struct Scheduler {
    threads: Vec<SharedRef<Thread>>,
}

impl Scheduler {
    pub const fn new() -> Scheduler {
        Scheduler {
            threads: Vec::new(),
        }
    }

    pub fn add_thread(&mut self, thread: SharedRef<Thread>) {
        self.threads.push(thread);
    }

    pub fn schedule(&mut self) -> Option<SharedRef<Thread>> {
        self.threads.pop()
    }
}

static SCHEDULER: GiantLock<Scheduler> = GiantLock::new(Scheduler::new());

fn idle() {
    // TODO:
    panic!("No runnable thread");
}

pub fn yield_to_user() {
    let next = SCHEDULER.borrow_mut().schedule();
    match next {
        Some(thread) => {
            // FIXME: set current thread here
            // FIXME: no locking
            Thread::switch_to(&thread);
        }
        None => {
            idle();
        }
    }
}

pub fn add_thread(thread: SharedRef<Thread>) {
    SCHEDULER.borrow_mut().add_thread(thread);
}
