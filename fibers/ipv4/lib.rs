#![no_std]

use ftl::channel::Message;
use ftl::event_queue::{Event, EventQueue, Interest};
use ftl::warn;

struct Deps {
    arp: ftl::channel::Channel,
}

struct Environ {
    deps: Deps,
}

#[derive(Debug)]
enum Context {
    Arp,
}

fn main(env: Environ) {
    let mut eventq = EventQueue::new();
    eventq
        .register_channel(&env.deps.arp, Interest::MESSAGE, Context::Arp)
        .unwrap();

    // for (state, event) in eventq.iter() {
    while let Some((state, event)) = eventq.next() {
        match (state, event) {
            (Context::Arp, Event::Message(Message::Packet { .. })) => {
                // eventq.register_channel(todo!(),
                todo!();
            }
            (state, event) => {
                warn!("unexpected event: {:?}, {:?}", state, event);
            }
        }
    }
}
