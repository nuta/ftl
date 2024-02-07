use core::{
    fmt,
    sync::atomic::{AtomicUsize, Ordering},
};

use alloc::{boxed::Box, sync::Arc};

use crate::{
    arch::{self, yield_cpu},
    sync::{channel::RawChannel, mutex::Mutex},
    task::scheduler::Scheduler,
};

use super::scheduler::GLOBAL_SCHEDULER;

enum State {
    Runnable,
    Blocked,
}

pub(crate) struct RawFiber {
    id: FiberId,
    state: State,
    ctx: arch::Context,
}

impl RawFiber {
    pub fn new_idle() -> Self {
        Self {
            id: FiberId::alloc(),
            state: State::Blocked,
            ctx: arch::Context::new_idle(),
        }
    }

    pub fn new_kernel(id: FiberId, pc: usize, arg: usize) -> Self {
        Self {
            id,
            state: State::Runnable,
            ctx: arch::Context::new_kernel(pc, arg),
        }
    }

    pub fn id(&self) -> FiberId {
        self.id
    }

    pub fn is_runnable(&self) -> bool {
        matches!(self.state, State::Runnable)
    }

    pub unsafe fn context_mut_ptr(&mut self) -> *mut arch::Context {
        &mut self.ctx as *mut arch::Context
    }

    pub fn resume_if_blocked(&mut self) {
        if matches!(self.state, State::Blocked) {
            self.state = State::Runnable;
            GLOBAL_SCHEDULER.lock().resume(self.id);
        }
    }

    pub fn block(&mut self) {
        debug_assert!(matches!(self.state, State::Runnable));
        self.state = State::Blocked;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FiberId(usize);

impl FiberId {
    pub fn alloc() -> FiberId {
        // TODO: wrap around and check for duplicates
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl fmt::Display for FiberId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

pub struct Fiber {
    raw: Arc<Mutex<RawFiber>>,
}

impl Fiber {
    pub fn spawn<F>(f: F) -> Fiber
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        Fiber::do_spawn(Box::new(f))
    }

    fn do_spawn(f: Box<dyn FnOnce()>) -> Fiber {
        let id = FiberId::alloc();

        extern "C" fn native_entry(arg: *mut Box<dyn FnOnce()>) {
            let closure = unsafe { Box::from_raw(arg) };
            closure();
            Scheduler::exit_current(GLOBAL_SCHEDULER.lock());
        }

        let main = move || {
            f();
            println!("fiber {} exited", id);
        };

        let pc = native_entry as usize;
        let closure = Box::into_raw(Box::new(main));
        let arg = closure as usize;
        let raw = Arc::new(Mutex::new(RawFiber::new_kernel(id, pc, arg)));

        GLOBAL_SCHEDULER.lock().add(raw.clone());

        Self { raw }
    }
}
