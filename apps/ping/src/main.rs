#![no_std]
#![no_main]

use ftl::application::Event;
use ftl::application::EventLoop;
use ftl::application::ReplyEvent;
use ftl::channel::Buffer;
use ftl::channel::Channel;
use ftl::channel::Message;
use ftl::handle::HandleId;
use ftl::handle::OwnedHandle;
use ftl::log::*;
use ftl::prelude::format;
use ftl::rc::Rc;

struct Main {
    counter: usize,
}

impl Main {
    fn new(eventloop: &mut EventLoop) -> Self {
        let ch_id = HandleId::from_raw(1);
        let ch = Rc::new(Channel::from_handle(OwnedHandle::from_raw(ch_id)));

        ch.send(Message::Write {
            offset: 0,
            data: Buffer::Static(b"Hello, world!"),
        })
        .unwrap();

        eventloop.add_channel(ch).unwrap();
        Self { counter: 0 }
    }

    fn on_write_reply(&mut self, ch: &Rc<Channel>, len: usize) {
        trace!("[ping] received write reply: {} bytes written", len);
        ch.send(Message::Write {
            offset: 0,
            data: Buffer::String(format!("Ping({})", self.counter)),
        })
        .unwrap();
        self.counter += 1;
    }
}

#[ftl::main]
fn main() {
    let mut eventloop = EventLoop::new().unwrap();
    let mut app = Main::new(&mut eventloop);

    loop {
        match eventloop.wait() {
            Event::Reply(ReplyEvent::Write { ch, buf: _, len }) => {
                app.on_write_reply(&ch, len);
            }
            ev => {
                warn!("[ping] unhandled event: {:?}", ev);
            }
        }
    }
}
