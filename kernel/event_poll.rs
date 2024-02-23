use alloc::collections::BTreeMap;
use alloc::sync::Arc;

use ftl_types::error::FtlError;
use ftl_types::event_poll::Event;
use ftl_types::handle::HandleId;

use crate::arch;
use crate::arch::cpuvar_ref;
use crate::fiber::Fiber;
use crate::lock::Mutex;
use crate::scheduler::GLOBAL_SCHEDULER;

struct RawEventPoll {
    pending: BTreeMap<isize /* Handle ID, but Ord */, Event>,
    receiver: Option<Arc<Mutex<Fiber>>>,
}

impl RawEventPoll {
    pub fn new() -> Self {
        Self {
            pending: BTreeMap::new(),
            receiver: None,
        }
    }

    pub fn notify(&mut self, handle_id: HandleId, event: Event) {
        *self
            .pending
            .entry(handle_id.as_isize())
            .or_insert_with(|| Event::zeroed()) |= event;

        if let Some(receiver) = self.receiver.take() {
            GLOBAL_SCHEDULER.lock().resume(receiver);
        }
    }

    pub fn poll(&mut self) -> Result<(HandleId, Event), FtlError> {
        loop {
            if let Some((handle, event)) = self.pending.pop_first() {
                return Ok((HandleId::from_isize(handle), event));
            }

            if self.receiver.is_some() {
                return Err(FtlError::InUse);
            }

            let current = cpuvar_ref().current.clone();
            GLOBAL_SCHEDULER.lock().block(&current);
            self.receiver = Some(current);
            arch::yield_cpu();
            self.receiver = None;
        }
    }
}

#[derive(Clone)]
pub struct EventPoll {
    raw: Arc<Mutex<RawEventPoll>>,
}

impl EventPoll {
    pub fn new() -> Self {
        Self {
            raw: Arc::new(Mutex::new(RawEventPoll::new())),
        }
    }

    pub fn notify(&self, handle_id: HandleId, event: Event) {
        self.raw.lock().notify(handle_id, event);
    }

    pub fn poll(&self) -> Result<(HandleId, Event), FtlError> {
        self.raw.lock().poll()
    }
}
