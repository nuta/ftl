use alloc::vec::Vec;

use crate::{
    cpuvar::{cpuvar, cpuvar_mut},
    giant_lock::GiantLock,
    ref_count::SharedRef,
    thread::{Thread, ThreadState},
};

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

fn idle() -> ! {
    // TODO:
    panic!("No runnable thread");
}

pub fn yield_to_user() -> ! {
    let next = {
        let mut scheduler = SCHEDULER.borrow_mut();

        let current = cpuvar_mut().current_thread.take();
        if let Some(thread) = current {
            if thread.borrow_mut().state() == ThreadState::Runnable {
                scheduler.add_thread(thread);
            }
        }

        scheduler.schedule()
    };

    match next {
        Some(thread) => {
            Thread::switch_to(thread);
        }
        None => {
            idle();
        }
    }
}

pub fn add_thread(thread: SharedRef<Thread>) {
    SCHEDULER.borrow_mut().add_thread(thread);
}
