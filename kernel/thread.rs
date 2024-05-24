use alloc::sync::Arc;

use crate::arch::cpuvar;
use crate::arch::{self};
use crate::scheduler::GLOBAL_SCHEDULER;

enum State {
    Runnable,
}

pub struct Thread {
    state: State,
    arch: arch::Thread,
}

impl Thread {
    pub fn new_idle() -> Thread {
        Thread {
            state: State::Runnable,
            arch: arch::Thread::new_idle(),
        }
    }

    pub fn spawn_kernel(pc: fn(usize), arg: usize) {
        let thread = Arc::new(Thread {
            state: State::Runnable,
            arch: arch::Thread::new_kernel(pc as usize, arg),
        });

        GLOBAL_SCHEDULER.push(thread);
    }

    pub fn is_runnable(&self) -> bool {
        matches!(self.state, State::Runnable)
    }

    pub const fn arch(&self) -> &arch::Thread {
        &self.arch
    }

    pub fn switch_to_this(&self) -> ! {
        self.arch.switch_to_this();
    }
}
