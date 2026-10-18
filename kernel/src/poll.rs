use alloc::collections::VecDeque;
use core::mem::MaybeUninit;
use core::mem::size_of;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::thread::SyscallRegs;
use ftl_types::time::MonoTime;
use ftl_utils::reserve_slot::ReserveSlot;
use ftl_utils::spinlock::SpinLock;

use crate::address::UAddr;
use crate::address::USlice;
use crate::arch;
use crate::handle::Handle;
use crate::handle::Handleable;
use crate::scheduler::SCHEDULER;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::Thread;
use crate::timer::GLOBAL_TIMER;

struct Mutable {
    queue: VecDeque<Cookie>,
    waiters: VecDeque<SharedRef<Thread>>,
    destroyed: bool,
}

pub struct Poll {
    /// Lock order: Lock [`GLOBAL_TIMER`] first, then this.
    mutable: SpinLock<Mutable>,
}

impl Poll {
    pub fn new() -> Self {
        Self {
            mutable: SpinLock::new(Mutable {
                queue: VecDeque::new(),
                waiters: VecDeque::new(),
                destroyed: false,
            }),
        }
    }

    fn enqueue(&self, cookie: Cookie) -> Result<(), ErrorCode> {
        let mut timer = GLOBAL_TIMER.lock();
        let mut mutable = self.mutable.lock();
        if mutable.destroyed {
            return Err(ErrorCode::Destroyed);
        }

        let slot = mutable
            .queue
            .reserve_slot()
            .map_err(|_| ErrorCode::OutOfMemory)?;

        slot.push_back(cookie);

        let Some(thread) = mutable.waiters.pop_front() else {
            return Ok(());
        };

        timer.cancel(&thread);
        SCHEDULER.push_back(thread);
        Ok(())
    }

    pub fn cancel(&self, thread: &SharedRef<Thread>) {
        let mut mutable = self.mutable.lock();
        mutable
            .waiters
            .retain(|waiter| !SharedRef::eq(waiter, thread));
    }

    pub fn notify(&self, cookie: usize) -> Result<(), ErrorCode> {
        self.enqueue(Cookie::from_raw(cookie)?)
    }

    pub fn try_wait(
        self: &SharedRef<Self>,
        thread: &SharedRef<Thread>,
        deadline: Option<MonoTime>,
    ) -> Result<Option<usize>, ErrorCode> {
        let mut timer = GLOBAL_TIMER.lock();
        let mut mutable = self.mutable.lock();

        if mutable.destroyed {
            return Err(ErrorCode::Destroyed);
        }

        if let Some(cookie) = mutable.queue.pop_front() {
            return Ok(Some(cookie.as_usize()));
        }

        if let Some(deadline) = deadline {
            // Check if the deadline has been reached.
            let now = arch::monotime_read();
            if now.duration_since(deadline).is_some() {
                return Err(ErrorCode::TimedOut);
            }
        }

        // Reserve a space for the new waiter.
        let slot = mutable
            .waiters
            .reserve_slot()
            .map_err(|_| ErrorCode::OutOfMemory)?;

        if let Some(deadline) = deadline {
            timer.add_poll_timeout(thread.clone(), deadline, self.clone())?;
        }

        // No events to return, enqueue the thread.
        slot.push_back(thread.clone());
        Ok(None)
    }
}

impl Handleable for Poll {
    fn close(self: SharedRef<Self>) {
        let mut timer = GLOBAL_TIMER.lock();
        let mut mutable = self.mutable.lock();
        mutable.destroyed = true;

        // Wake up threads waiting on this poll, and cancel their poll timeouts.
        for thread in mutable.waiters.drain(..) {
            timer.cancel(&thread);
            SCHEDULER.push_back(thread);
        }
    }
}

#[derive(Clone, Copy)]
struct Cookie(usize);

impl Cookie {
    fn from_raw(raw: usize) -> Result<Self, ErrorCode> {
        if raw > (isize::MAX as usize) {
            return Err(ErrorCode::InvalidArg);
        }

        Ok(Self(raw))
    }

    fn as_usize(self) -> usize {
        self.0
    }
}

pub struct EventEmitter {
    poll: SharedRef<Poll>,
    cookie: Cookie,
}

impl EventEmitter {
    pub fn new(poll: SharedRef<Poll>, cookie: usize) -> Result<Self, ErrorCode> {
        let cookie = Cookie::from_raw(cookie)?;
        Ok(Self { poll, cookie })
    }

    pub fn emit(&self) -> Result<(), ErrorCode> {
        self.poll.enqueue(self.cookie)
    }
}

pub fn sys_poll_create(
    current: &SharedRef<Thread>,
    _ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let poll = SharedRef::new(Poll::new())?;
    let handle = Handle::new(poll, HandleRight::READ | HandleRight::WRITE);
    let handle_id = current.hspace().insert(handle)?;
    Ok(SyscallOutput::Done(handle_id.as_usize()))
}

pub fn sys_poll_wait(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let handle_id = HandleId::new(ctx.a0);

    let poll = current.hspace().get::<Poll>(handle_id, HandleRight::READ)?;

    current.start_polling(poll, None)
}

pub fn sys_poll_wait_until(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let handle_id = HandleId::new(ctx.a0);
    let deadline_uslice = USlice::new(UAddr::new(ctx.a1), size_of::<MonoTime>())?;

    let poll = current.hspace().get::<Poll>(handle_id, HandleRight::READ)?;

    let mut deadline_buf = MaybeUninit::uninit();
    let deadline = unsafe { deadline_uslice.read_uninit(&mut deadline_buf)? };

    current.start_polling(poll, Some(*deadline))
}

pub fn sys_poll_notify(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let handle_id = HandleId::new(ctx.a0);
    let cookie = ctx.a1;

    current
        .hspace()
        .get::<Poll>(handle_id, HandleRight::WRITE)?
        .notify(cookie)?;

    Ok(SyscallOutput::Done(0))
}
