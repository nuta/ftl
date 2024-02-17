use ftl_types::{error::FtlError, event_poll::Event, handle::HandleId};

use crate::channel::Channel;

pub struct EventPoll {
    raw: ftl_kernel::event_poll::EventPoll,
}

impl EventPoll {
    pub fn new() -> Self {
        Self {
            raw: ftl_kernel::event_poll::EventPoll::new(),
        }
    }

    pub fn add_channel(&mut self, ch: &mut Channel) -> Result<(), FtlError> {
        let handle_id = ch.handle_id();
        ch.kernel_raw().poll_in(handle_id, &self.raw)
    }

    pub fn poll(&mut self) -> Result<(HandleId, Event), FtlError> {
        self.raw.poll()
    }
}
