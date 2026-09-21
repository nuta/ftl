use alloc::vec::Vec;

use ftl_types::error::ErrorCode;
use ftl_types::thread::SyscallRegs;
use ftl_types::time::MonoTime;
use ftl_types::time::WallTime;
use ftl_utils::reserve_slot::ReserveSlot;
use ftl_utils::spinlock::SpinLock;

use crate::address::UAddr;
use crate::address::USlice;
use crate::poll::Poll;
use crate::scheduler::SCHEDULER;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::Thread;

pub static GLOBAL_TIMER: SpinLock<Timer> = SpinLock::new(Timer::new());

struct Entry {
    thread: SharedRef<Thread>,
    deadline: MonoTime,
    poll: SharedRef<Poll>,
}

pub struct Timer {
    entries: Vec<Entry>,
}

impl Timer {
    const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add_poll_timeout(
        &mut self,
        thread: SharedRef<Thread>,
        deadline: MonoTime,
        poll: SharedRef<Poll>,
    ) -> Result<(), ErrorCode> {
        self.entries
            .reserve_slot()
            .map_err(|_| ErrorCode::OutOfMemory)?
            .push(Entry {
                deadline,
                thread,
                poll,
            });
        Ok(())
    }

    pub fn cancel(&mut self, thread: &SharedRef<Thread>) {
        self.entries
            .retain(|entry| !SharedRef::eq(&entry.thread, thread));
    }

    pub fn tick(&mut self, now: MonoTime) {
        // TODO: Avoid scanning the entire entries.
        self.entries.retain(|entry| {
            if now.duration_since(entry.deadline).is_none() {
                // The deadline has not been reached yet.
                return true;
            }

            // Wake up the thread and remove it from the waiters list.
            entry.poll.cancel(&entry.thread);
            SCHEDULER.push_back(entry.thread.clone());
            false
        });
    }
}

pub fn sys_monotime_read(
    _current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let now = crate::arch::monotime_read();
    let output = USlice::new(UAddr::new(ctx.a0), size_of::<MonoTime>())?;
    output.write(now)?;
    Ok(SyscallOutput::Done(0))
}

pub fn sys_walltime_read(
    _current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let now = crate::arch::walltime_read();
    let output = USlice::new(UAddr::new(ctx.a0), size_of::<WallTime>())?;
    output.write(now)?;
    Ok(SyscallOutput::Done(0))
}
