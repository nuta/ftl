use alloc::sync::Arc;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;

use crate::arch::{self};
use crate::scheduler::GLOBAL_SCHEDULER;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreadId(usize);

impl ThreadId {
    pub fn new_idle() -> ThreadId {
        ThreadId(0)
    }

    pub fn alloc() -> ThreadId {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        ThreadId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

enum State {
    Runnable,
}

pub struct Thread {
    id: ThreadId,
    state: State,
    arch: arch::Thread,
}

impl Thread {
    pub fn new_idle() -> Thread {
        Thread {
            id: ThreadId::new_idle(),
            state: State::Runnable,
            arch: arch::Thread::new_idle(),
        }
    }

    pub fn spawn_kernel(pc: fn(usize), arg: usize) {
        let thread = Arc::new(Thread {
            id: ThreadId::alloc(),
            state: State::Runnable,
            arch: arch::Thread::new_kernel(pc as usize, arg),
        });

        GLOBAL_SCHEDULER.push(thread);
    }

    pub fn id(&self) -> ThreadId {
        self.id
    }

    pub fn is_idle_thread(&self) -> bool {
        self.id.0 == 0
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
