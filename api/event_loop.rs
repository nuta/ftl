use ftl_types::error::FtlError;
use ftl_types::event_poll::Event as RawEvent;
use ftl_types::handle::HandleId;
use ftl_types::message::MessageOrSignal;
use ftl_types::Message;
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

pub enum Event {
    ChannelReceived {
        channel: &mut Channel,
        message: MessageOrSignal,
    },
}

pub struct Eventloop<State> {
    event_poll: EventPoll,
    pending: Option<(HandleId, RawEvent)>,
    objects: HashMap<HandleId, Object<State>>,
}

impl<State> Eventloop<State> {
    pub fn new() -> Self {
        Self {
            event_poll: EventPoll::new(),
            pending: None,
            objects: HashMap::new(),
        }
    }

    pub fn add_channel(&mut self, mut ch: Channel, state: State) -> Result<(), FtlError> {
        self.event_poll.add_channel(&mut ch)?;

        let handle_id = ch.handle_id();
        let object = Object {
            kind: ObjectKind::Channel(ch),
            state,
        };

        self.objects.insert(handle_id, object);
        Ok(())
    }

    pub fn next_event(&mut self) -> Result<(&mut State, Event), FtlError> {
        let (handle_id, mut raw_event) = match self.pending {
            Some((handle_id, raw_event)) => (handle_id, raw_event),
            None => self.event_poll.poll()?,
        };

        if raw_event.is_empty() {
            unreachable!("this should not happen");
        }

        let object = self.objects.get_mut(&handle_id).unwrap();
        let event = match &mut object.kind {
            ObjectKind::Channel(ch) => {
                if raw_event.is_readable() {
                    raw_event.unset(RawEvent::READABLE);

                    // TODO: how should we handle receive errors?
                    let message = ch.receive().unwrap();
                    Event::ChannelReceived {
                        channel: ch,
                        message,
                    }
                } else {
                    todo!("consume_event: unhandled event: {:?}", raw_event);
                }
            }
        };

        if raw_event.is_empty() {
            self.pending = None;
        } else {
            self.pending = Some((handle_id, raw_event));
        }

        Ok((&mut object.state, event))
    }
}
