use alloc::{collections::VecDeque, sync::Arc};
use hashbrown::HashMap;
use spin::{Lazy, MutexGuard};

use crate::{
    arch::{self, cpuvar_mut, cpuvar_ref},
    sync::mutex::Mutex,
    task::fiber::RawFiber,
};

use super::fiber::FiberId;

pub(crate) static GLOBAL_SCHEDULER: Lazy<Mutex<Scheduler>> =
    Lazy::new(|| Mutex::new(Scheduler::new()));

pub(crate) struct Scheduler {
    fibers: HashMap<FiberId, Arc<Mutex<RawFiber>>>,
    run_queue: VecDeque<FiberId>,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            fibers: HashMap::new(),
            run_queue: VecDeque::new(),
        }
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
        let current_id = cpuvar_ref().current.lock().id();
        guard.fibers.remove(&current_id);
        guard.run_queue.retain(|id| *id != current_id);
        cpuvar_mut().current = cpuvar_mut().idle.clone();

        Self::switch_to_next(guard);
    }

    pub fn switch_to_next<'a>(mut guard: MutexGuard<'a, Self>) -> ! {
        {
            let current = cpuvar_ref().current.lock();
            if current.is_runnable() {
                guard.run_queue.push_back(current.id());
            }

            let Some(next_id) = guard.run_queue.pop_front() else {
                todo!("no fibers to run")
                // TODO: return idle thread
            };

            assert!(next_id != current.id());

            let next_lock = guard.fibers.get(&next_id).unwrap().clone();
            let mut next = next_lock.lock();
            println!("switching to fiber {}", next.id());
            let cpuvar = cpuvar_mut();
            cpuvar.context = unsafe { next.context_mut_ptr() };
            drop(next);
            cpuvar_mut().current = next_lock;
        }

        drop(guard);
        arch::restore_context();
    }
}
