use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;

use crate::channel::Channel;
use crate::poll::Poll;
use crate::poll::{self};

pub enum Message {
    Foo,
}

#[derive(Debug)]
pub enum Error {
    Poll(FtlError),
    UntrackedHandle(HandleId),
}

pub enum Object {
    Channel(Channel),
}

pub enum Event<'a, State> {
    Message(&'a mut State, &'a mut Channel, Message),
}

pub struct Mainloop<State> {
    poll: Poll,
    states: HashMap<HandleId, (Object, State)>,
}

impl<State> Mainloop<State> {
    pub fn new() -> Mainloop<State> {
        Mainloop {
            poll: Poll::new().unwrap(),
            states: HashMap::new(),
        }
    }

    pub fn add(&mut self, id: HandleId, state: State) -> Result<(), Error> {
        self.poll.add(id).map_err(Error::Poll)?;
        self.states.insert(id, todo!());
        Ok(())
    }

    pub fn next(&mut self) -> Result<Option<Event<State>>, Error> {
        let (poll_ev, id) = self.poll.wait().map_err(Error::Poll)?;
        let Some((object, state)) = self.states.get_mut(&id) else {
            return Err(Error::UntrackedHandle(id));
        };

        let ev = match object {
            Object::Channel(channel) => {
                match poll_ev {
                    poll::Event::ChannelNewMessage => Event::Message(state, channel, Message::Foo),
                }
            }
        };

        Ok(Some(ev))
    }
}

fn main() {
    struct St {
        foo: isize,
    }
    let mut mainloop = Mainloop::<St>::new();
    while let Some(ev) = mainloop.next().unwrap() {
        match ev {
            Event::Message(st, ch, m) => {
                // TODO:
            }
        }
    }
}
