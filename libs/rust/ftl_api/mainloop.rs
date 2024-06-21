use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;

use crate::channel::Channel;
use crate::handle::Handleable;
use crate::poll::Poll;
use crate::poll::{self};
use crate::println;

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
    PollError(Error),
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

    pub fn add_channel(&mut self, ch: Channel, state: State) -> Result<(), Error> {
        let id = ch.handle_id();
        self.poll.add(id).map_err(Error::Poll)?;
        self.states.insert(id, (Object::Channel(ch), state));
        Ok(())
    }

    pub fn next(&mut self) -> Option<Event<State>> {
        let (poll_ev, id) = self.poll.wait().map_err(Error::Poll).ok()?;
        let Some((object, state)) = self.states.get_mut(&id) else {
            return Some(Event::PollError(Error::UntrackedHandle(id)));
        };

        let ev = match object {
            Object::Channel(channel) => {
                match poll_ev {
                    poll::Event::ChannelNewMessage => Event::Message(state, channel, Message::Foo),
                }
            }
        };

        Some(ev)
    }
}

fn main() {
    struct St {
        foo: isize,
    }

    let mut mainloop = Mainloop::<St>::new();
    while let Some(ev) = mainloop.next() {
        match ev {
            Event::Message(st, ch, m) => {
                match m {
                    Message::Foo => {
                        println!("ch handle ID: {:?}", ch.handle_id());
                        st.foo += 1;
                        println!("Foo: {}", st.foo);
                        println!("ch handle ID: {:?}", ch.handle_id());
                    }
                }
            }
            Event::PollError(err) => {
                println!("Error: {:?}", err);
            }
        }
    }
}
