use alloc::{collections::BTreeMap, sync::Arc};
use ftl_types::{error::FtlError, event_poll::Event, handle::HandleId};

use crate::{
    arch::{self, cpuvar_ref},
    fiber::Fiber,
    lock::Mutex,
    scheduler::GLOBAL_SCHEDULER,
};

pub struct EventPoll {
    pending: BTreeMap<isize /* Handle ID, but Ord */, Event>,
    receiver: Option<Arc<Mutex<Fiber>>>,
}

impl EventPoll {
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

    pub fn poll(&mut self) -> Result<Option<(HandleId, Event)>, FtlError> {
        loop {
            if let Some((handle, event)) = self.pending.pop_first() {
                return Ok(Some((HandleId::new(handle), event)));
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
