use alloc::collections::VecDeque;
use core::mem::MaybeUninit;
use core::mem::size_of;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::poll::Event;
use ftl_types::poll::EventKind;
use ftl_types::thread::SyscallRegs;
use ftl_types::time::MonoTime;
use ftl_utils::spinlock::SpinLock;

use crate::address::UAddr;
use crate::address::USlice;
use crate::arch;
use crate::handle::Handle;
use crate::handle::Handleable;
use crate::scheduler::SCHEDULER;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::CurrentThread;
use crate::thread::Thread;
use crate::timer::GLOBAL_TIMER;

struct Mutable {
    queue: VecDeque<Event>,
    waiters: VecDeque<SharedRef<Thread>>,
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
            }),
        }
    }

    fn enqueue(&self, event: Event) -> Result<(), ErrorCode> {
        let mut timer = GLOBAL_TIMER.lock();
        let mut mutable = self.mutable.lock();
        mutable
            .queue
            .try_reserve(1)
            .map_err(|_| ErrorCode::OutOfMemory)?;

        mutable.queue.push_back(event);

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

    pub fn notify(&self, self_id: HandleId) -> Result<(), ErrorCode> {
        self.enqueue(Event::new(EventKind::PollNotified, self_id))
    }

    pub fn try_wait(
        self: &SharedRef<Self>,
        thread: &SharedRef<Thread>,
        poll_id: HandleId,
        deadline: Option<MonoTime>,
    ) -> Result<Option<Event>, ErrorCode> {
        let mut timer = GLOBAL_TIMER.lock();
        let mut mutable = self.mutable.lock();

        if let Some(event) = mutable.queue.pop_front() {
            return Ok(Some(event));
        }

        if let Some(deadline) = deadline {
            // Check if the deadline has been reached.
            let now = arch::monotime_read();
            if now.duration_since(deadline).is_some() {
                let event = Event::new(EventKind::PollTimeout, poll_id);
                return Ok(Some(event));
            }
        }

        // Reserve a space for the new waiter.
        mutable
            .waiters
            .try_reserve(1)
            .map_err(|_| ErrorCode::OutOfMemory)?;

        if let Some(deadline) = deadline {
            timer.add_poll_timeout(thread.clone(), deadline, self.clone())?;
        }

        // No events to return, enqueue the thread.
        mutable.waiters.push_back(thread.clone());
        Ok(None)
    }
}

impl Handleable for Poll {}

pub struct EventEmitter {
    poll: SharedRef<Poll>,
    handle_id: HandleId,
}

impl EventEmitter {
    pub fn new(poll: SharedRef<Poll>, handle_id: HandleId) -> Self {
        Self { poll, handle_id }
    }

    pub fn emit(&self, kind: EventKind) -> Result<(), ErrorCode> {
        self.poll.enqueue(Event::new(kind, self.handle_id))
    }
}

pub fn sys_poll_create(
    current: &SharedRef<Thread>,
    _ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let poll = SharedRef::new(Poll::new())?;
    let handle = Handle::new(poll, HandleRight::READ | HandleRight::WRITE);
    let handle_id = current.isolate().handles().lock().insert(handle)?;
    Ok(SyscallOutput::Done(handle_id.as_usize()))
}

pub fn sys_poll_wait(
    current: &SharedRef<Thread>,
    current_thread: &CurrentThread,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let handle_id = HandleId::new(ctx.a0);

    let poll = current
        .isolate()
        .handles()
        .lock()
        .get::<Poll>(handle_id, HandleRight::READ)?;

    current.start_polling(current_thread, poll, handle_id, None)
}

pub fn sys_poll_wait_until(
    current: &SharedRef<Thread>,
    current_thread: &CurrentThread,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let handle_id = HandleId::new(ctx.a0);
    let deadline_uslice = USlice::new(UAddr::new(ctx.a1), size_of::<MonoTime>())?;

    let poll = current
        .isolate()
        .handles()
        .lock()
        .get::<Poll>(handle_id, HandleRight::READ)?;

    let mut deadline_buf = MaybeUninit::uninit();
    let deadline = unsafe { deadline_uslice.read_uninit(&mut deadline_buf)? };

    current.start_polling(current_thread, poll, handle_id, Some(*deadline))
}

pub fn sys_poll_notify(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let handle_id = HandleId::new(ctx.a0);

    current
        .isolate()
        .handles()
        .lock()
        .get::<Poll>(handle_id, HandleRight::WRITE)?
        .notify(handle_id)?;

    Ok(SyscallOutput::Done(0))
}
