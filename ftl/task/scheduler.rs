use alloc::{collections::VecDeque, sync::Arc};
use hashbrown::HashMap;
use spin::{Lazy, MutexGuard};

use crate::{
    arch::{self, cpuvar_mut},
    sync::mutex::Mutex,
    task::fiber::RawFiber,
};

use super::fiber::FiberId;

pub(crate) static GLOBAL_SCHEDULER: Lazy<Mutex<Scheduler>> =
    Lazy::new(|| Mutex::new(Scheduler::new()));

pub(crate) struct Scheduler {
    current: Option<Arc<Mutex<RawFiber>>>,
    fibers: HashMap<FiberId, Arc<Mutex<RawFiber>>>,
    run_queue: VecDeque<FiberId>,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            current: None,
            fibers: HashMap::new(),
            run_queue: VecDeque::new(),
        }
    }

    // FIXME: Don't clone
    pub fn current(&self) -> Arc<Mutex<RawFiber>> {
        self.current.clone().unwrap()
    }

    pub fn add(&mut self, fiber: Arc<Mutex<RawFiber>>) {
        let id = fiber.lock().id();
        self.run_queue.push_back(id);
        self.fibers.insert(id, fiber);
    }

    pub fn resume(&mut self, id: FiberId) {
        self.run_queue.push_back(id);
    }

    pub fn exit_current<'a>(mut guard: MutexGuard<'a, Self>) -> ! {
        let current_id = guard.current.as_ref().unwrap().lock().id();
        guard.fibers.remove(&current_id);
        guard.run_queue.retain(|id| *id != current_id);
        guard.current = None;

        Self::switch_to_next(guard);
    }

    pub fn switch_to_next<'a>(mut guard: MutexGuard<'a, Self>) -> ! {
        if let Some(current_lock) = guard.current.take() {
            let current = current_lock.lock();
            if current.is_runnable() {
                guard.run_queue.push_back(current.id());
            }
        }

        let Some(next_id) = guard.run_queue.pop_front() else {
            todo!("no fibers to run")
        };

        {
            let next_lock = guard.fibers.get(&next_id).unwrap().clone();
            let mut next = next_lock.lock();
            println!("switching to fiber {}", next.id());
            let cpuvar = cpuvar_mut();
            cpuvar.context = unsafe { next.context_mut_ptr() };
            guard.current = Some(next_lock.clone());
            drop(guard);
        }

        arch::restore_context();
    }
}
