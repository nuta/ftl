use core::{
    fmt,
    sync::atomic::{AtomicUsize, Ordering},
};

use alloc::{boxed::Box, sync::Arc};

use crate::{arch, sync::mutex::Mutex};

use super::scheduler::GLOBAL_SCHEDULER;

enum State {
    Runnable,
    Blocked,
}

struct Inner {
    state: State,
    ctx: arch::Context,
}

pub(crate) struct KernelFiber {
    id: FiberId,
    inner: Mutex<Inner>,
}

impl KernelFiber {
    pub fn new_kernel(id: FiberId, pc: usize, arg: usize) -> Self {
        Self {
            id,
            inner: Mutex::new(Inner {
                state: State::Runnable,
                ctx: arch::Context::new_kernel(pc, arg),
            }),
        }
    }

    pub fn id(&self) -> FiberId {
        self.id
    }

    pub fn is_runnable(&self) -> bool {
        matches!(self.inner.lock().state, State::Runnable)
    }

    pub fn resume_if_blocked(&self) {
        let mut inner = self.inner.lock();
        if matches!(inner.state, State::Blocked) {
            inner.state = State::Runnable;
            GLOBAL_SCHEDULER.resume(self.id);
        }
    }

    pub fn block(&self) {
        let mut inner = self.inner.lock();
        debug_assert!(matches!(inner.state, State::Runnable));
        inner.state = State::Blocked;
    }

    pub unsafe fn context_mut_ptr(&self) -> *mut arch::Context {
        let mut inner = self.inner.lock();
        &mut inner.ctx as *mut arch::Context
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

pub struct Fiber {}

impl Fiber {
    pub fn spawn<F>(f: F)
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        Fiber::do_spawn(Box::new(f))
    }

    fn do_spawn(f: Box<dyn FnOnce()>) {
        let id = FiberId::alloc();

        extern "C" fn native_entry(arg: *mut Box<dyn FnOnce()>) {
            let closure = unsafe { Box::from_raw(arg) };
            closure();
            GLOBAL_SCHEDULER.exit_current();
        }

        let main = move || {
            f();
            println!("fiber {} exited", id);
        };

        let pc = native_entry as usize;
        let closure = Box::into_raw(Box::new(main));
        let arg = closure as usize;
        let raw = Arc::new(KernelFiber::new_kernel(id, pc, arg));
        GLOBAL_SCHEDULER.add(raw.clone());
    }
}
