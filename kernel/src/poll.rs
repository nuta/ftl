use alloc::collections::VecDeque;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::poll::Event;
use ftl_types::poll::EventKind;
use ftl_types::thread::SyscallRegs;
use ftl_utils::spinlock::SpinLock;

use crate::handle::Handle;
use crate::handle::Handleable;
use crate::scheduler::SCHEDULER;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::CurrentThread;
use crate::thread::Thread;

struct Mutable {
    queue: VecDeque<Event>,
    waiters: VecDeque<SharedRef<Thread>>,
}

pub struct Poll {
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
        let mut mutable = self.mutable.lock();
        mutable
            .queue
            .try_reserve(1)
            .map_err(|_| ErrorCode::OUT_OF_MEMORY)?;

        mutable.queue.push_back(event);

        let Some(thread) = mutable.waiters.pop_front() else {
            return Ok(());
        };

        // FIXME: How can we ensure this won't fail?
        SCHEDULER.push_back(thread.clone())?;
        Ok(())
    }

    pub fn notify(&self, self_id: HandleId) -> Result<(), ErrorCode> {
        self.enqueue(Event::new(EventKind::PollNotified, self_id))
    }

    pub fn try_wait(&self, thread: &SharedRef<Thread>) -> Result<Option<Event>, ErrorCode> {
        let mut mutable = self.mutable.lock();

        let Some(event) = mutable.queue.pop_front() else {
            mutable
                .waiters
                .try_reserve(1)
                .map_err(|_| ErrorCode::OUT_OF_MEMORY)?;
            mutable.waiters.push_back(thread.clone());
            return Ok(None);
        };

        Ok(Some(event))
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

    current.start_polling(current_thread, poll)
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
