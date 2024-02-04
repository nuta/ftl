use core::mem;

use alloc::{collections::VecDeque, sync::Arc};

use crate::{sync::mutex::Mutex, task::fiber::RawFiber};

pub static GLOBAL_SCHEDULER: Scheduler = Scheduler::new();

struct Inner {
    current: Option<Arc<Mutex<RawFiber>>>,
    run_queue: VecDeque<Arc<Mutex<RawFiber>>>,
}

pub struct Scheduler {
    inner: Mutex<Inner>,
}

pub(crate) fn after_restore() {
    unsafe {
        let inner = GLOBAL_SCHEDULER.inner.lock();
        let current = inner.current.as_ref().unwrap();
        current.force_unlock();
    }
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

    pub fn switch(&self) {
        let mut inner = self.inner.lock();
        if let Some(current) = inner.current.take() {
            inner.run_queue.push_back(current);
        }

        if let Some(next) = inner.run_queue.pop_front() {
            inner.current = Some(next.clone());
            drop(inner);

            let mut next = next.lock();
            next.restore();

            // We'll release the lock when the fiber is resumed.
            mem::forget(next);
        }
    }
}
