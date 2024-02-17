use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;

use crate::channel::Channel;
use crate::event_poll::EventPoll;

enum ObjectKind {
    Channel(Channel),
}

struct Object<State> {
    kind: ObjectKind,
    state: State,
}

pub enum Event<'a> {
    ChannelReceived { channel: &'a mut Channel },
    ChannelClosed { channel: &'a mut Channel },
}

pub struct Eventloop<State> {
    poll: EventPoll,
    objects: HashMap<HandleId, Object<State>>,
}

impl<State> Eventloop<State> {
    pub fn new() -> Self {
        Self {
            poll: EventPoll::new(),
            objects: HashMap::new(),
        }
    }

    pub fn add_channel(&mut self, mut ch: Channel, state: State) -> Result<(), FtlError> {
        self.poll.add_channel(&mut ch)?;

        let handle_id = ch.handle_id();
        let object = Object {
            kind: ObjectKind::Channel(ch),
            state,
        };

        self.objects.insert(handle_id, object);
        Ok(())
    }

    pub fn run<F>(mut self, f: F)
    where
        F: Fn(&mut Eventloop<State>, &mut State, Event<'_>),
    {
        todo!()
    }
}
