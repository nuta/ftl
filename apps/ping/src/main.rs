#![no_std]
#![no_main]

use ftl::channel::Buffer;
use ftl::channel::Channel;
use ftl::eventloop::Client;
use ftl::eventloop::Event;
use ftl::eventloop::EventLoop;
use ftl::handle::HandleId;
use ftl::handle::OwnedHandle;
use ftl::log::*;
use ftl::prelude::format;
use ftl::rc::Rc;

struct Main {
    client: Client<()>,
    counter: usize,
}

impl Main {
    fn new(eventloop: &mut EventLoop<(), ()>) -> Self {
        let ch_id = HandleId::from_raw(1);
        let ch = Rc::new(Channel::from_handle(OwnedHandle::from_raw(ch_id)));
        let client = eventloop.add_channel(ch, ()).unwrap();

        client.write(0, Buffer::Static(b"Hello, world!"), ()).unwrap();

        Self { client, counter: 0 }
    }

    fn on_write_reply(&mut self, len: usize) {
        trace!("[ping] received write reply: {} bytes written", len);
        self.client
            .write(
                0,
                Buffer::String(format!("Ping({})", self.counter)),
                (),
            )
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
            Event::WriteReply { len, .. } => {
                app.on_write_reply(len);
            }
            ev => {
                warn!("[ping] unhandled event: {:?}", ev);
            }
        }
    }
}
