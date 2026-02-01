#![no_std]
#![no_main]

use ftl::application::Application;
use ftl::application::Context;
use ftl::application::ReadCompleter;
use ftl::application::WriteCompleter;
use ftl::channel::Channel;
use ftl::channel::Reply;
use ftl::handle::HandleId;
use ftl::handle::OwnedHandle;
use ftl::println;
use ftl::sink::Event;
use ftl::sink::Sink;

struct Main {}

impl Application for Main {
    fn init(ctx: &mut Context) -> Self {
        let ch_id = HandleId::from_raw(1);
        let ch = Channel::from_handle(OwnedHandle::from_raw(ch_id));
        ctx.add_channel(ch).unwrap();
        Self {}
    }

    fn write(&mut self, ctx: &mut Context, completer: WriteCompleter, offset: usize, len: usize) {
        let mut buf = [0; 512];
        let len = completer.read_data(offset, &mut buf).unwrap();
        println!(
            "[pong] OOL read ({len} bytes): {:?}",
            core::str::from_utf8(&buf[..len])
        );
        completer.complete(len);
    }
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::run::<Main>();
}
