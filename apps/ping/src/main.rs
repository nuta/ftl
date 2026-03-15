#![no_std]
#![no_main]

use ftl::channel::Channel;
use ftl::eventloop::Event;
use ftl::eventloop::EventLoop;
use ftl::eventloop::Reply;
use ftl::log::*;
use ftl::prelude::format;
use ftl::rc::Rc;

#[ftl::main]
fn main() {
    let mut eventloop = EventLoop::new().unwrap();

    let ch = Rc::new(Channel::connect("pong").unwrap());
    let client = eventloop.add_channel(ch, 0).unwrap();

    client.write(0, b"Hello, world!".as_slice(), ()).unwrap();
    loop {
        match eventloop.wait() {
            Event::Reply {
                reply: Reply::Write { .. },
                ctx: counter,
                ..
            } => {
                if *counter >= 10 {
                    info!("done!");
                } else {
                    client.write(0, format!("Ping({})", counter), ()).unwrap();
                    *counter += 1;
                }
            }
            ev => {
                warn!("[ping] unhandled event: {:?}", ev);
            }
        }
    }
}
