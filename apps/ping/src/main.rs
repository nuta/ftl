#![no_std]
#![no_main]

use ftl::application::Application;
use ftl::application::Context;
use ftl::channel::Buffer;
use ftl::channel::Channel;
use ftl::channel::Message;
use ftl::handle::HandleId;
use ftl::handle::OwnedHandle;
use ftl::prelude::*;
use ftl::println;
use ftl::rc::Rc;

struct Main {
    counter: usize,
}

impl Application for Main {
    fn init(ctx: &mut Context) -> Self {
        let ch_id = HandleId::from_raw(1);
        let ch = Channel::from_handle(OwnedHandle::from_raw(ch_id));

        ch.send(Message::Write {
            offset: 0,
            data: Buffer::Static(b"Hello, world!"),
        })
        .unwrap();

        ctx.add_channel(ch).unwrap();
        Self { counter: 0 }
    }

    fn write_reply(&mut self, _ctx: &mut Context, ch: &Rc<Channel>, _buf: Buffer, len: usize) {
        println!("[ping] received write reply: {} bytes written", len);
        ch.send(Message::Write {
            offset: 0,
            data: Buffer::String(format!("Ping({})", self.counter)),
        })
        .unwrap();
        self.counter += 1;
    }
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::run::<Main>();
}
