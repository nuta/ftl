use alloc::collections::btree_set::BTreeSet;
use alloc::collections::vec_deque::VecDeque;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::sink::Event;
use ftl_types::sink::EventHeader;

use crate::handle::HandleRight;
use crate::handle::Handleable;
use crate::isolation::Isolation;
use crate::isolation::UserPtr;
use crate::isolation::UserSlice;
use crate::process::HandleTable;
use crate::shared_ref::SharedRef;
use crate::spinlock::SpinLock;
use crate::syscall::SyscallResult;
use crate::thread::Promise;
use crate::thread::Thread;

pub struct EventEmitter {
    sink: SharedRef<Sink>,
    id: HandleId,
}

impl EventEmitter {
    pub fn new(sink: SharedRef<Sink>, id: HandleId) -> Self {
        Self { sink, id }
    }

    pub fn notify(&self) {
        self.sink.enqueue(self.id);
    }
}

struct Mutable {
    /// The queue of handles IDs that are ready to be notified.
    ready_queue: VecDeque<usize>,
    ready_set: BTreeSet<usize>,
    waiters: VecDeque<SharedRef<Thread>>,
}

pub struct Sink {
    mutable: SpinLock<Mutable>,
}

impl Sink {
    pub fn new() -> Result<SharedRef<Self>, ErrorCode> {
        SharedRef::new(Self {
            mutable: SpinLock::new(Mutable {
                ready_queue: VecDeque::new(),
                ready_set: BTreeSet::new(),
                waiters: VecDeque::new(),
            }),
        })
    }

    fn add<T: Handleable + ?Sized>(
        self: &SharedRef<Self>,
        id: HandleId,
        object: SharedRef<T>,
    ) -> Result<(), ErrorCode> {
        object.set_event_emitter(Some(EventEmitter::new(self.clone(), id)))?;

        let mut mutable = self.mutable.lock();
        mutable.ready_queue.push_back(id.as_usize());
        mutable.ready_set.insert(id.as_usize());
        Ok(())
    }

    fn enqueue(&self, id: HandleId) {
        let mut mutable = self.mutable.lock();
        if mutable.ready_set.contains(&id.as_usize()) {
            // It's already in the queue.
            return;
        }

        mutable.ready_queue.push_back(id.as_usize());
        mutable.ready_set.insert(id.as_usize());

        if let Some(waiter) = mutable.waiters.pop_front() {
            waiter.unblock();
        }
    }

    pub fn wait(
        self: &SharedRef<Self>,
        current: &SharedRef<Thread>,
        isolation: &SharedRef<dyn Isolation>,
        handle_table: &mut HandleTable,
        buf: &UserSlice,
    ) -> Result<bool, ErrorCode> {
        let mut mutable = self.mutable.lock();
        while let Some(id) = mutable.ready_queue.pop_front() {
            mutable.ready_set.remove(&id);

            let handle_id = HandleId::from_raw(id);
            let Some(object) = handle_table.get_any(handle_id) else {
                // The object has been removed from the handle table.
                // TODO: What if the ID is reused?
                continue;
            };

            // TODO: This authorize is not necessary because we already checked
            //       when adding the object to the sink.
            let handle = object.authorize(HandleRight::READ)?;
            let Some((ty, event)) = handle.read_event(handle_table)? else {
                // The object has no events to report.
                continue;
            };

            let header = EventHeader { ty, id: handle_id };
            crate::isolation::write(isolation, buf, 0, header)?;
            crate::isolation::write(isolation, buf, size_of::<EventHeader>(), event)?;

            return Ok(true);
        }

        mutable.waiters.push_back(current.clone());
        Ok(false)
    }
}

impl Handleable for Sink {}

pub fn sys_sink_add(current: &SharedRef<Thread>, a0: usize, a1: usize) -> Result<usize, ErrorCode> {
    let sink_id = HandleId::from_raw(a0);
    let object_id = HandleId::from_raw(a1);

    let process = current.process();
    let handle_table = process.handle_table().lock();

    let object = handle_table
        .get_any(object_id)
        .ok_or(ErrorCode::HandleNotFound)?
        .authorize(HandleRight::READ)?;

    handle_table
        .get::<Sink>(sink_id)?
        .authorize(HandleRight::WRITE)?
        .add(object_id, object)?;

    Ok(0)
}

pub fn sys_sink_wait(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
) -> Result<SyscallResult, ErrorCode> {
    let sink_id = HandleId::from_raw(a0);
    let buf = UserSlice::new(
        UserPtr::new(a1),
        size_of::<EventHeader>() + size_of::<Event>(),
    )?;

    let process = current.process();
    let mut handle_table = process.handle_table().lock();
    let sink = handle_table
        .get::<Sink>(sink_id)?
        .authorize(HandleRight::READ)?;

    let done = sink.wait(current, process.isolation(), &mut handle_table, &buf)?;
    if done {
        Ok(SyscallResult::Return(0))
    } else {
        Ok(SyscallResult::Blocked(Promise::SinkWait { sink, buf }))
    }
}
