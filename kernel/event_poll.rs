use ftl_types::{event_poll::Event, handle::HandleId};
use hashbrown::HashMap;

pub struct EventPoll {
    pending: HashMap<HandleId, Event>,
}

impl EventPoll {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    pub fn add_event(&mut self, handle_id: HandleId, event: Event) {
        let e = self
            .pending
            .entry(handle_id)
            .or_insert_with(|| Event::zeroed());

        *e |= event;
    }

    pub fn poll(&mut self) -> Option<(HandleId, Event)> {
        self.pending.iter().next().map(|(k, v)| (*k, v.clone()))
    }
}
