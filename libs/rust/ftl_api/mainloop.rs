use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;
use ftl_types::message::MessageBody;
use ftl_types::message::MessageBuffer;
use ftl_types::poll::PollEvent;
use hashbrown::HashMap;

use crate::channel::Channel;
use crate::channel::RecvError;
use crate::poll::Poll;
use crate::println;

#[derive(Debug)]
pub enum Error {
    PollCreate(FtlError),
    PollWait(FtlError),
    ChannelRecv(RecvError),
}

#[derive(Debug)]
pub enum Event<'a, St, M: MessageBody> {
    Message { state: &'a mut St, message: M::Reader<'a> },
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

impl<St> Mainloop<St> {
    pub fn new() -> Result<Self, Error> {
        let poll = Poll::new().map_err(Error::PollCreate)?;

        Ok(Self {
            poll,
            msgbuffer: MessageBuffer::new(),
            objects: HashMap::new(),
        })
    }

    pub fn next<M: MessageBody>(&mut self) -> Result<Event<'_, St, M>, Error> {
        let (poll_ev, handle_id) = self.poll.wait().map_err(Error::PollWait)?;
        let entry = self.objects.get_mut(&handle_id).unwrap();
        if poll_ev.contains(PollEvent::READABLE) {
            match &entry.object {
                Object::Channel(channel) => {
                    let message = channel
                        .recv_with_buffer::<M>(&mut self.msgbuffer)
                        .map_err(Error::ChannelRecv)?;
                    return Ok(Event::Message {
                        state: &mut entry.state,
                        message,
                    });
                }
            }
        }

        todo!("unhandled poll event: {:?}", poll_ev);
    }
}

fn main() {
    pub struct State {}
    pub struct Msg {}
    pub struct MsgReader<'a> {
        buffer: &'a MessageBuffer,
    }
    impl MessageBody for Msg {
        const MSGINFO: ftl_types::message::MessageInfo = ftl_types::message::MessageInfo::from_raw(123);
        type Reader<'a> = MsgReader<'a>;
        fn deserialize<'a>(buffer: &'a MessageBuffer) -> Self::Reader<'a> {
            todo!()
        }
    }

    let mut mainloop: Mainloop<State> = Mainloop::new().unwrap();
    loop {
        match mainloop.next::<Msg>() {
            Ok(Event::Message { state, message }) => {
                println!("Received message");
            }
            Err(err) => {
                println!("Error: {:?}", err);
            }
        }
    }
}
