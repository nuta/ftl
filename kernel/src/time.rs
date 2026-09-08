use alloc::vec::Vec;

use ftl_types::error::ErrorCode;
use ftl_types::poll::EventKind;
use ftl_types::thread::SyscallRegs;
use ftl_types::time::MonoTime;
use ftl_utils::spinlock::SpinLock;

use crate::address::UAddr;
use crate::address::USlice;
use crate::poll::EventEmitter;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::Thread;

pub static GLOBAL_TIMER: Timer = Timer::new();

struct Entry {
    deadline: MonoTime,
    emitter: EventEmitter,
}

pub struct Timer {
    entries: SpinLock<Vec<Entry>>,
}

impl Timer {
    const fn new() -> Self {
        Self {
            entries: SpinLock::new(Vec::new()),
        }
    }

    pub fn add(&self, deadline: MonoTime, emitter: EventEmitter) -> Result<(), ErrorCode> {
        let mut entries = self.entries.lock();
        entries.try_reserve(1).map_err(|_| ErrorCode::OutOfMemory)?;
        entries.push(Entry { deadline, emitter });
        Ok(())
    }

    pub fn tick(&self, now: MonoTime) {
        let mut entries = self.entries.lock();
        entries.retain(|entry| {
            let expired = now >= entry.deadline;
            if expired {
                if let Err(e) = entry.emitter.emit(EventKind::PollTimeout) {
                    trace!("failed to emit timeout event: {:?}", e);
                }
            }

            !expired
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
