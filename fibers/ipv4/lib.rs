#![no_std]

use ftl::channel::Message;
use ftl::event_queue::{Event, EventQueue, Interest};
use ftl::warn;

pub struct Deps {
    arp: ftl::channel::Channel,
}

pub struct Environ {
    deps: Deps,
}

#[derive(Debug)]
enum Context {
    Arp,
}

pub fn main(env: Environ) {
    let mut eventq = EventQueue::new();
    eventq
        .register_channel(&env.deps.arp, Interest::MESSAGE, Context::Arp)
        .unwrap();

    while let Some((state, event)) = eventq.next() {
        match (state, event) {
            (Context::Arp, Event::Message(Message::Packet { .. })) => {
                todo!();
            }
            (state, event) => {
                warn!("unexpected event: {:?}, {:?}", state, event);
            }
        }
    }
}
