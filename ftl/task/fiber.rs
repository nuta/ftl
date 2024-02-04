use alloc::sync::Arc;

use crate::{
    arch,
    sync::{channel::RawChannel, mutex::Mutex},
};

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
    pub fn new(pc: usize, sp: usize) -> Self {
        Self {
            state: State::Runnable,
            ctx: arch::Context::new(pc, sp),
        }
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

impl Fiber {}
