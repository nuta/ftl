use alloc::{collections::VecDeque, sync::Arc};
use hashbrown::HashMap;
use spin::Lazy;

use crate::{
    arch::{self, cpuvar_mut},
    sync::mutex::Mutex,
};

use super::fiber::{FiberId, KernelFiber};

pub(crate) static GLOBAL_SCHEDULER: Lazy<Scheduler> = Lazy::new(|| Scheduler::new());

struct Inner {
    current: Option<Arc<KernelFiber>>,
    fibers: HashMap<FiberId, Arc<KernelFiber>>,
    run_queue: VecDeque<FiberId>,
}

pub(crate) struct Scheduler {
    inner: Mutex<Inner>,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        let inner = Inner {
            current: None,
            fibers: HashMap::new(),
            run_queue: VecDeque::new(),
        };

        Scheduler {
            inner: Mutex::new(inner),
        }
    }

    // FIXME: Don't clone
    pub fn current(&self) -> Arc<KernelFiber> {
        self.inner.lock().current.clone().unwrap()
    }

    pub fn add(&self, fiber: Arc<KernelFiber>) {
        let mut inner = self.inner.lock();
        let id = fiber.id();
        inner.run_queue.push_back(id);
        inner.fibers.insert(id, fiber);
    }

    pub fn resume(&self, id: FiberId) {
        let mut inner = self.inner.lock();
        inner.run_queue.push_back(id);
    }

    pub fn exit_current(&self) -> ! {
        {
            let mut inner = self.inner.lock();
            let current_id = inner.current.as_ref().unwrap().id();
            inner.fibers.remove(&current_id);
            inner.run_queue.retain(|id| *id != current_id);
            inner.current = None;
        }
        self.switch_to_next();
    }

    pub fn switch_to_next(&self) -> ! {
        let mut inner = self.inner.lock();
        if let Some(current) = inner.current.take() {
            if current.is_runnable() {
                inner.run_queue.push_back(current.id());
            }
        }

        let Some(next_id) = inner.run_queue.pop_front() else {
            todo!("no fibers to run")
        };

        {
            let next = inner.fibers.get(&next_id).unwrap().clone();
            println!("switching to fiber {}", next.id());
            let cpuvar = cpuvar_mut();
            cpuvar.context = unsafe { next.context_mut_ptr() };
            inner.current = Some(next.clone());
            drop(inner);
        }

        arch::restore_context();
    }
}
