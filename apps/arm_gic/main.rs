#![no_std]
#![no_main]

use ftl_api::folio::MmioFolio;
use ftl_api::mainloop::Mainloop;
use ftl_api::prelude::*;
use ftl_api::types::address::PAddr;
use ftl_api::types::address::VAddr;
use ftl_api::types::environ::Device;
use ftl_api::types::message::MessageBuffer;
use ftl_api_autogen::apps::arm_gic::Environ;
use ftl_api_autogen::apps::arm_gic::Message;

enum Context {
    Autopilot,
}

#[ftl_api::main]
pub fn main(mut env: Environ) {
    info!("starting arm_gic: {:?}", env.depends.gic);
    let mut mainloop = Mainloop::<Context, Message>::new().unwrap();
    mainloop
        .add_channel(env.autopilot_ch.take().unwrap(), Context::Autopilot)
        .unwrap();

    let mut buffer = MessageBuffer::new();
    loop {
        match mainloop.next(&mut buffer) {
            _ => {
                warn!("unhandled event");
            }
        }
    }
}
