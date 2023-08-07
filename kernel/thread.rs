use crate::{arch::{self, PageTable}, address::UAddr, ref_count::{SharedRef, UniqueRef}, process::Process};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Runnable,
    Blocked,
}

pub struct Thread {
    process: SharedRef<Process>,
    state: ThreadState,
    context: arch::Context,
}

impl Thread {
    pub fn new(process: SharedRef<Process>, pc: UAddr) -> Thread {
        Thread {
            process,
            state: ThreadState::Blocked,
            context: arch::Context::new_user(pc),
        }
    }

    pub fn switch_to_this(&self) {
        self.context.switch_to_this(self.process.borrow_mut().page_table());
    }
}
