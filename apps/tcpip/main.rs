#![no_std]
#![no_main]

use ftl_api::channel::Channel;
use ftl_api::handle::OwnedHandle;
use ftl_api::mainloop::Event;
use ftl_api::mainloop::Mainloop;
use ftl_api::prelude::*;
use ftl_api::types::idl::StringField;
use ftl_api::types::message::MessageBuffer;
use ftl_api_autogen::apps::tcpip::Environ;
use ftl_api_autogen::apps::tcpip::Message;
use ftl_api_autogen::protocols::ping::PingReply;

enum Context {
    Autopilot,
    Client { counter: i32 },
}

#[ftl_api::main]
pub fn main(mut env: Environ) {
    info!("starting...");

    let mut mainloop = Mainloop::<Context, Message>::new().unwrap();
    mainloop
        .add_channel(env.autopilot_ch.take().unwrap(), Context::Autopilot)
        .unwrap();

    let mut buffer = MessageBuffer::new();
    loop {
        match mainloop.next(&mut buffer) {
            Event::Message { ch, ctx, m } => {
                match (ctx, m) {
                    (Context::Autopilot, Message::NewclientRequest(m)) => {
                        info!("got new client: {:?}", m.handle());
                        let new_ch = Channel::from_handle(OwnedHandle::from_raw(m.handle()));
                        mainloop
                            .add_channel(new_ch, Context::Client { counter: 0 })
                            .unwrap();
                    }
                    // (Context::Client { counter }, _) => {
                    // }
                    _ => {
                        // TODO: dump message with fmt::Debug
                        panic!("unknown message");
                    }
                }
            }
            _ => {
                panic!("unexpected event");
            }
            Event::Error(err) => {
                panic!("mainloop error: {:?}", err);
            }
        }
    }
}
