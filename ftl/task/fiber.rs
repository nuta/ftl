use alloc::sync::Arc;

use crate::sync::{channel::RawChannel, mutex::Mutex};

enum BlockedBy {
    ChannelReceive(Arc<Mutex<RawChannel>>),
}

enum State {
    Runnable,
    Blocked(BlockedBy),
}

pub(crate) struct RawFiber {
    state: State,
    // ctx: arch::Context,
}

impl RawFiber {
    pub fn new() -> Self {
        Self {
            state: State::Runnable,
            // ctx: arch::Context::new(),
        }
    }

    pub fn resume_if_blocked(&mut self) {
        if matches!(self.state, State::Blocked(_)) {
            self.state = State::Runnable;
        }
    }
}
