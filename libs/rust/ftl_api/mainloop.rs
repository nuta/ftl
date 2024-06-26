use core::marker::PhantomData;

use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;
use ftl_types::message::MessageBody;
use ftl_types::message::MessageBuffer;
use ftl_types::message::MessageInfo;
use ftl_types::poll::PollEvent;
use hashbrown::HashMap;

use crate::channel::Channel;
use crate::channel::RecvError;
use crate::handle::OwnedHandle;
use crate::poll::Poll;
use crate::println;

#[derive(Debug)]
pub enum Error {
    PollCreate(FtlError),
    PollAdd(FtlError),
    PollWait(FtlError),
    ChannelRecv(RecvError),
    ChannelAlreadyAdded(Channel),
}

#[derive(Debug)]
pub enum Event<'a, St, M: MessageBody> {
    Message {
        ch: &'a mut Channel,
        state: &'a mut St,
        message: M::Reader<'a>,
    },
}

enum Object {
    Channel(Channel),
}

struct Entry<St> {
    state: St,
    object: Object,
}

pub struct Mainloop<St, AllM> {
    poll: Poll,
    msgbuffer: MessageBuffer,
    objects: HashMap<HandleId, Entry<St>>,
    _pd: PhantomData<AllM>,
}

impl<St, AllM: MessageBody> Mainloop<St, AllM> {
    pub fn new() -> Result<Self, Error> {
        let poll = Poll::new().map_err(Error::PollCreate)?;

        Ok(Self {
            poll,
            msgbuffer: MessageBuffer::new(),
            objects: HashMap::new(),
            _pd: PhantomData,
        })
    }

    pub fn add_channel(&mut self, ch: Channel, state: St) -> Result<(), Error> {
        let handle_id = ch.handle().id();
        if self.objects.contains_key(&handle_id) {
            return Err(Error::ChannelAlreadyAdded(ch));
        }

        let entry = Entry {
            state,
            object: Object::Channel(ch),
        };

        self.objects.insert(handle_id, entry);
        self.poll
            .add(handle_id, PollEvent::READABLE)
            .map_err(Error::PollAdd)?;

        Ok(())
    }

    pub fn next(&mut self) -> Result<Event<'_, St, AllM>, Error> {
        let (poll_ev, handle_id) = self.poll.wait().map_err(Error::PollWait)?;
        let entry = self.objects.get_mut(&handle_id).unwrap();
        if poll_ev.contains(PollEvent::READABLE) {
            match &mut entry.object {
                Object::Channel(ch) => {
                    let message = ch
                        .recv_with_buffer::<AllM>(&mut self.msgbuffer)
                        .map_err(Error::ChannelRecv)?;
                    return Ok(Event::Message {
                        ch,
                        state: &mut entry.state,
                        message,
                    });
                }
            }
        }

        todo!("unhandled poll event: {:?}", poll_ev);
    }
}

pub fn main() {
    pub struct State {
        value: i32,
    }

    pub enum AllMessage<'a> {
        Foo(FooMsgReader<'a>),
    }

    impl<'b> MessageBody for AllMessage<'b> {
        const MSGINFO: ftl_types::message::MessageInfo =
            ftl_types::message::MessageInfo::from_raw(0);
        type Reader<'a> = AllMessage<'a>;
        fn deserialize<'a>(
            buffer: &'a MessageBuffer,
            msginfo: MessageInfo,
        ) -> Option<AllMessage<'a>> {
            match msginfo {
                FooMsg::MSGINFO => {
                    let reader = FooMsg::deserialize(buffer, msginfo)?;
                    Some(AllMessage::Foo(reader))
                }
                _ => None,
            }
        }
    }

    pub struct FooMsg {}
    pub struct FooMsgReader<'a> {
        buffer: &'a MessageBuffer,
    }
    impl<'a> FooMsgReader<'a> {
        fn hello(&self) -> u8 {
            self.buffer.data[0]
        }
    }
    impl MessageBody for FooMsg {
        const MSGINFO: ftl_types::message::MessageInfo =
            ftl_types::message::MessageInfo::from_raw(123);
        type Reader<'a> = FooMsgReader<'a>;
        fn deserialize<'a>(
            buffer: &'a MessageBuffer,
            _msginfo: MessageInfo,
        ) -> Option<Self::Reader<'a>> {
            Some(FooMsgReader { buffer })
        }
    }

    let mut mainloop: Mainloop<State, AllMessage> = Mainloop::new().unwrap();
    loop {
        match mainloop.next() {
            Ok(Event::Message { ch, state, message }) => {
                println!("Received message");
                match message {
                    AllMessage::Foo(m) => {
                        println!("Foo: {}", m.hello());
                        state.value += 1;
                        ch.send_with_buffer(&mut MessageBuffer::new(), FooMsg {}).unwrap();
                        mainloop.add_channel(Channel::from_handle(OwnedHandle::from_raw(HandleId::from_raw(2))), State { value: 123}).unwrap();
                    }
                }
            }
            Err(err) => {
                println!("Error: {:?}", err);
            }
        }
    }
}
