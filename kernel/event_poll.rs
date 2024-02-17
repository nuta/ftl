use ftl_types::{event_poll::Event, handle::HandleId};
use hashbrown::HashMap;

pub struct EventPoll {
    pending: HashMap<HandleId, Event>,
}
