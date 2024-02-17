pub struct EventPoll {
    raw: ftl_kernel::event_poll::EventPoll,
}

impl EventPoll {
    pub fn new() -> Self {
        Self {
            raw: ftl_kernel::event_poll::EventPoll::new(),
        }
    }

    pub fn poll(&mut self) -> Result<(HandleId, Event), FtlError> {
        self.raw.poll()
    }
}
