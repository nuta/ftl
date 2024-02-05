use core::mem::ManuallyDrop;

use alloc::{collections::VecDeque, sync::Arc};

use crate::{
    arch::{self, cpuvar_mut},
    sync::mutex::Mutex,
    task::fiber::RawFiber,
};

pub static GLOBAL_SCHEDULER: Scheduler = Scheduler::new();

struct Inner {
    current: Option<Arc<Mutex<RawFiber>>>,
    run_queue: VecDeque<Arc<Mutex<RawFiber>>>,
}

pub struct Scheduler {
    inner: Mutex<Inner>,
}

impl Scheduler {
    pub const fn new() -> Scheduler {
        let inner = Inner {
            current: None,
            run_queue: VecDeque::new(),
        };

        Scheduler {
            inner: Mutex::new(inner),
        }
    }

    pub(crate) fn add(&self, fiber: Arc<Mutex<RawFiber>>) {
        self.inner.lock().run_queue.push_back(fiber);
    }

    pub fn switch_to_next(&self) -> ! {
        let mut inner = self.inner.lock();
        if let Some(current) = inner.current.take() {
            if current.lock().is_runnable() {
                inner.run_queue.push_back(current);
            }
        }

        let Some(next_lock) = inner.run_queue.pop_front() else {
            todo!("no fibers to run")
        };

        {
            let mut next = next_lock.lock();
            let cpuvar = cpuvar_mut();
            cpuvar.context = unsafe { next.context_mut_ptr() };
            inner.current = Some(next_lock.clone());
            drop(inner);
        }

        arch::restore_context();
    }
}
