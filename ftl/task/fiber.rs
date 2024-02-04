use alloc::sync::Arc;

use crate::sync::{channel::RawChannel, mutex::Mutex};

enum BlockedBy {
    ChannelReceive(Arc<Mutex<RawChannel>>),
}

enum State {
    Running,
    Blocked(BlockedBy),
}

pub(crate) struct RawFiber {
    state: State,
    // ctx: arch::Context,
}
