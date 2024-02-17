use alloc::{collections::VecDeque, sync::Arc};
use spin::{Lazy, MutexGuard};

use crate::{
    arch::{self, cpuvar_mut, cpuvar_ref},
    fiber::Fiber,
    lock::Mutex,
};

use super::fiber::FiberState;

pub(crate) static GLOBAL_SCHEDULER: Lazy<Mutex<Scheduler>> =
    Lazy::new(|| Mutex::new(Scheduler::new()));

pub(crate) struct Scheduler {
    run_queue: VecDeque<Arc<Mutex<Fiber>>>,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            run_queue: VecDeque::new(),
        }
    }

    pub fn resume(&mut self, fiber: Arc<Mutex<Fiber>>) {
        fiber.lock().set_state(FiberState::Runnable);
        self.run_queue.push_back(fiber);
    }

    pub fn block(&mut self, fiber: &Arc<Mutex<Fiber>>) {
        fiber.lock().set_state(FiberState::Blocked);
    }

    pub fn exit_current<'a>(guard: MutexGuard<'a, Self>) -> ! {
        Self::do_switch_to_next(guard);
    }

    pub fn switch_to_next<'a>(mut guard: MutexGuard<'a, Self>) -> ! {
        {
            let current_lock = &cpuvar_ref().current;
            let current = current_lock.lock();
            if matches!(current.state(), FiberState::Runnable) {
                if guard.run_queue.is_empty() {
                    // FIXME: No other runnable fibers other than the current one.
                    todo!();
                }

                guard.run_queue.push_back(current_lock.clone());
            }
        }

        Self::do_switch_to_next(guard);
    }

    pub fn do_switch_to_next<'a>(mut guard: MutexGuard<'a, Self>) -> ! {
        {
            let next_lock = match guard.run_queue.pop_front() {
                Some(fiber) => fiber,
                None => cpuvar_ref().idle.clone(),
            };

            // TODO: make sure it's not the same fiber
            // assert!(next_id != current.id());

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
