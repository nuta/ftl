use alloc::{boxed::Box, sync::Arc};

use crate::{
    arch,
    sync::{channel::RawChannel, mutex::Mutex},
};

use super::scheduler::GLOBAL_SCHEDULER;

enum BlockedBy {
    ChannelReceive(Arc<Mutex<RawChannel>>),
}

enum State {
    Runnable,
    Blocked(BlockedBy),
}

pub(crate) struct RawFiber {
    state: State,
    ctx: arch::Context,
}

impl RawFiber {
    pub fn new_kernel(pc: usize, arg: usize) -> Self {
        Self {
            state: State::Runnable,
            ctx: arch::Context::new_kernel(pc, arg),
        }
    }

    pub fn is_runnable(&self) -> bool {
        matches!(self.state, State::Runnable)
    }

    pub unsafe fn context_mut_ptr(&mut self) -> *mut arch::Context {
        &mut self.ctx as *mut arch::Context
    }

    pub fn resume_if_blocked(&mut self) {
        if matches!(self.state, State::Blocked(_)) {
            self.state = State::Runnable;
        }
    }
}

pub struct Fiber {
    raw: Arc<Mutex<RawFiber>>,
}

extern "C" fn native_entry(arg: *mut Box<dyn FnOnce()>) {
    println!("native_entry");
    let closure = unsafe { Box::from_raw(arg) };
    closure();
}

impl Fiber {
    pub fn spawn<F>(f: F) -> Fiber
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        Fiber::do_spawn(Box::new(f))
    }

    fn do_spawn(f: Box<dyn FnOnce()>) -> Fiber {
        let pc = native_entry as usize;
        let closure = Box::into_raw(Box::new(f));
        let arg = closure as usize;
        println!("pc: {:#x}, arg: {:#x}", pc, closure as usize);
        let raw = Arc::new(Mutex::new(RawFiber::new_kernel(pc, arg)));

        GLOBAL_SCHEDULER.add(raw.clone());

        Self { raw }
    }
}
