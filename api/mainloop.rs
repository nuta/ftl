use alloc::vec::Vec;

use ftl_types::error::FtlError;
use ftl_types::event_poll::Event as RawEvent;
use ftl_types::handle::HandleId;
use ftl_types::message::MessageOrSignal;
use hashbrown::HashMap;

use crate::channel::Channel;
use crate::channel::Receiver;
use crate::channel::Sender;
use crate::event_poll::EventPoll;

enum Object {
    Channel { sender: Sender, receiver: Receiver },
}

struct Entry<State> {
    object: Object,
    state: State,
}

pub enum Event<'a> {
    ChannelReceived {
        sender: &'a mut Sender,
        message: MessageOrSignal,
    },
}

pub struct Mainloop<State> {
    event_poll: EventPoll,
    pending: Option<(HandleId, RawEvent)>,
    objects: HashMap<HandleId, Entry<State>>,
}

impl<State> Mainloop<State> {
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
        let (sender, receiver) = ch.split();
        let entry = Entry {
            object: Object::Channel { sender, receiver },
            state,
        };

        self.objects.insert(handle_id, entry);
        Ok(())
    }

    fn next_event(&mut self) -> Result<(&mut State, Event<'_>), FtlError> {
        let (handle_id, mut raw_event) = match self.pending {
            Some((handle_id, raw_event)) => (handle_id, raw_event),
            None => self.event_poll.poll()?,
        };

        if raw_event.is_empty() {
            unreachable!("this should not happen");
        }

        let entry = self.objects.get_mut(&handle_id).unwrap();
        let event = match &mut entry.object {
            Object::Channel { sender, receiver } => {
                if raw_event.is_readable() {
                    raw_event.unset(RawEvent::READABLE);

                    // TODO: how should we handle receive errors?
                    let message = receiver.receive().unwrap();
                    Event::ChannelReceived { sender, message }
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

        Ok((&mut entry.state, event))
    }

    fn apply_changes(&mut self, changes: &mut Changes<State>) {
        for command in changes.commands.drain(..) {
            match command {
                Command::AddChannel(ch, state) => {
                    if let Err(err) = self.add_channel(ch, state) {
                        panic!("mainloop: failed to add channel: {:?}", err);
                    }
                }
                Command::RemoveChannel(handle_id) => {
                    self.objects.remove(&handle_id);
                }
            }
        }
    }

    /// # Why `Send` is required?
    ///
    /// To allow running the callback function in multiple threads in the
    /// future!
    pub fn run<F>(&mut self, f: F)
    where
        F: Fn(&mut Changes<State>, &mut State, Event<'_>) + Send + 'static,
    {
        let mut changes = Changes::new();
        loop {
            let (state, event) = self
                .next_event()
                .expect("mainloop: failed to get next event");

            f(&mut changes, state, event);
            self.apply_changes(&mut changes);
        }
    }
}

enum Command<State> {
    AddChannel(Channel, State),
    RemoveChannel(HandleId),
}

/// Due to the borrow checker, we can't pass `&mut Mainloop` to the callback
/// function, along with the `&mut State` which is a part of the `Mainloop`.
///
/// This object allows the callback to queue changes to the mainloop's state.
pub struct Changes<State> {
    commands: Vec<Command<State>>,
}

impl<State> Changes<State> {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.commands.clear();
    }

    pub fn add_channel(&mut self, ch: Channel, state: State) {
        self.commands.push(Command::AddChannel(ch, state));
    }

    pub fn remove_channel(&mut self, ch: &Channel) {
        self.commands.push(Command::RemoveChannel(ch.handle_id()));
    }
}
