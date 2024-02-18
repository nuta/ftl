#![no_std]
#![allow(unused)] // FIXME:

use ftl_api::channel::Channel;
use ftl_api::collections::HashMap;
use ftl_api::device::mmio::ReadWrite;
use ftl_api::environ::Environ;
use ftl_api::folio::Folio;
use ftl_api::mainloop::Event;
use ftl_api::mainloop::Mainloop;
use ftl_api::prelude::*;
use ftl_api::sync::Arc;
use ftl_api::sync::SpinLock;
use ftl_api::types::address::PAddr;
use ftl_api::types::message::Message;
use ftl_api::types::message::MessageOrSignal;
use ftl_api::types::signal::Signal;

pub fn main(env: Environ) {
    println!("virtio_net: starting: {:?}", env.device());
    let base_paddr = PAddr::new(env.device().reg as usize).unwrap();

    // TODO: EventPoll to handle enable_irq requests
    let mut eventloop = Mainloop::new();
    eventloop.add_channel(todo!(), ()).unwrap();
    eventloop.run(|_, state, event| {
        todo!();
        todo!();
    });
}
