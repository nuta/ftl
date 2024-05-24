use alloc::sync::Arc;

use crate::{arch::{self, cpuvar}, scheduler::GLOBAL_SCHEDULER};

pub struct Thread {
    arch: arch::Thread,
}

impl Thread {
    pub fn new_idle() -> Thread {
        Thread {
            arch: arch::Thread::new_idle(),
        }
    }

    pub fn spawn_kernel(pc: &'static fn(usize), arg: usize) {
        let thread = Arc::new(Thread {
            arch: arch::Thread::new_kernel(pc as *const _ as usize, arg),
        });

        GLOBAL_SCHEDULER.push(thread);
    }

    pub const fn arch(&self) -> &arch::Thread {
        &self.arch
    }

    pub fn switch_to_this(&self) -> ! {
        self.arch.switch_to_this();
    }
}
