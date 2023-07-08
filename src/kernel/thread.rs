use crate::{arch};


#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Runnable,
    Blocked,
}

pub struct Thread {
    state: ThreadState,
    context: arch::Context,
}
