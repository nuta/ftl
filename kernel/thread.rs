use crate::arch::{self, cpuvar};

pub struct Thread {
    arch: arch::Thread,
}

impl Thread {
    pub fn new_idle() -> Thread {
        Thread {
            arch: arch::Thread::new_idle(),
        }
    }

    pub fn new_kernel(pc: &'static fn(usize), arg: usize) -> Thread {
        Thread {
            arch: arch::Thread::new_kernel(pc as *const _ as usize, arg),
        }
    }

    pub const fn arch(&self) -> &arch::Thread {
        &self.arch
    }
}
