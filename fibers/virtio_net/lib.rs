#![no_std]
#![allow(unused)] // FIXME:

use ftl_api::environ::Environ;
use ftl_api::mainloop::Event;
use ftl_api::mainloop::Mainloop;
use ftl_api::prelude::*;

pub fn main(env: Environ) {
    println!("virtio_net: starting: {:?}", env.device());

    // TODO: EventPoll to handle enable_irq requests
    let mut eventloop = Mainloop::new();
    eventloop.add_channel(todo!(), ()).unwrap();
    eventloop.run(|_, state, event| {
        todo!();
        todo!();
    });
}
