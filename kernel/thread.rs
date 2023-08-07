use crate::{arch::{self, PageTable}, address::UAddr, ref_count::{SharedRef, UniqueRef}, process::Process, cpuvar::cpuvar_mut};

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

    pub fn state(&self) -> ThreadState {
        self.state
    }

    pub fn block(&mut self) {
        debug_assert!(self.state != ThreadState::Blocked);

        self.state = ThreadState::Blocked;
    }

    pub fn resume(&mut self) {
        debug_assert!(self.state != ThreadState::Runnable);

        self.state = ThreadState::Runnable;
    }

    pub fn switch_to(this: SharedRef<Thread>) -> ! {
        let thread = unsafe { SharedRef::force_borrow(&this).as_ref() };
        cpuvar_mut().current_thread = Some(this);
        thread.context.switch_to_this();
    }
}
