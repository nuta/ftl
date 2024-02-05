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
    pub fn new(pc: usize, sp: usize, arg: usize) -> Self {
        Self {
            state: State::Runnable,
            ctx: arch::Context::new(pc, sp, arg),
        }
    }

    /// # `inline(always)` is essential!
    ///
    /// The `inline(always)` attribute is essential for this method to
    /// get the correct return address.
    #[inline(always)]
    pub fn save(&mut self) {
        self.ctx.save();
    }

    pub fn restore(&mut self) -> ! {
        self.ctx.restore();
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

fn native_entry(arg: usize) {
    let closure = unsafe { Box::from_raw(arg as *mut Box<dyn FnOnce()>) };
    closure();
    todo!("fiber has returned to native_entry");
}

impl Fiber {
    pub fn spawn<F>(f: F) -> Fiber
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        let closure = Box::new(f);
        let pc = native_entry as usize;
        let sp = 0;
        let arg = Box::into_raw(closure) as usize;
        let raw = Arc::new(Mutex::new(RawFiber::new(pc, sp, arg)));

        GLOBAL_SCHEDULER.add(raw.clone());

        Self { raw }
    }
}
