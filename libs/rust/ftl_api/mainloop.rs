use ftl_types::{error::FtlError, handle::HandleId, message::MessageBuffer, poll::PollEvent};
use hashbrown::HashMap;

use crate::{channel::Channel, poll::Poll};

#[derive(Debug)]
pub enum Error {
    PollWait(FtlError),
}

#[derive(Debug)]
pub enum Event<'a, St, M> {
    Message {
        state: &'a mut St,
        message: M,
    }
}

enum Object {
    Channel(Channel),
}

struct Entry<St> {
    state: St,
    object: Object,
}

pub struct Mainloop<St> {
    poll: Poll,
    msgbuffer: MessageBuffer,
    objects: HashMap<HandleId, Entry<St>>,
}

impl<St, M> Mainloop<St, M> {
    pub fn new() -> Self {
        Self {
            poll: Poll::new(),
            msgbuffer: MessageBuffer::new(),
            objects: HashMap::new(),
        }
    }

    pub fn next(&mut self) -> Result<Event<'_>, Error> {
        let (poll_ev, handle_id) = self.poll.wait().map_err(Error::PollWait)?;
        let entry = self.objects.get_mut(&handle_id).unwrap();
        if poll_ev.contains(PollEvent::READABLE) {
            match &entry.object {
                Object::Channel(channel) => {
                    let message = channel.recv_with_buffer(&mut self.msgbuffer).map_err(Error::PollWait)?;
                    return Some(Ok(Event::Message {
                        state: &mut entry.state,
                        message,
                    }));
                }
            }
        }
    }
}

fn main() {
    let mut mainloop = Mainloop::new();
    loop {
        match mainloop.next() {
            Ok(Event::Message { state, message }) => {
                println!("Received message: {:?}", message);
            }
            Err(err) => {
                println!("Error: {:?}", err);
            }
        }
    }
}
