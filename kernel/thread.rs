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
        let context = arch::Context::new_user(&process, pc);
        Thread {
            process,
            state: ThreadState::Blocked,
            context,
        }
    }

    pub fn switch_to_this(&self) {
        // FIXME: don't borrow!
        self.context.switch_to_this();
    }
}
