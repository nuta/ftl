#![no_std]

use ftl_api::channel::Channel;
use ftl_api::environ::Environ;
use ftl_api::handle::Handle;
use ftl_api::mainloop::Event;
use ftl_api::mainloop::Mainloop;
use ftl_api::prelude::*;
use ftl_api::types::message::Message;
use ftl_autogen::fibers::riscv_plic::Deps;

#[derive(Debug)]
enum State {
    Autopilot,
    Client,
}

struct Virtio {}

impl Virtio {
    pub fn new() -> Self {
        Virtio {}
    }

    pub fn send(&self, pkt: &[u8]) {
        todo!()
    }
}

pub fn main(mut env: Environ) {
    let deps: Deps = env.parse_deps().expect("failed to parse deps");
    let virtio = Virtio::new();

    let mut mainloop = Mainloop::new();
    mainloop
        .add_channel(deps.autopilot, State::Autopilot)
        .unwrap();

    mainloop.run(move |changes, state, event| {
        match (state, event) {
            (State::Autopilot, Event::Message(_, Message::NewClient { ch: handle })) => {
                let ch = Channel::from_handle(Handle::new(handle));
                changes.add_channel(ch, State::Client);
            }
            (State::Client, Event::Message(_, Message::NetworkTx(pkt))) => {
                virtio.send(&pkt);
            }
            (_state, _event) => {
                todo!();
            }
        }
    });
}
