#![no_std]
#![no_main]

use ftl_api::prelude::*;
use ftl_api::types::message::MessageBuffer;
use ftl_api_autogen::apps::ping::Environ;
use ftl_api_autogen::protocols::ping::PingReply;
use ftl_api_autogen::protocols::ping::PingRequest;

#[ftl_api::main]
pub fn main(mut env: Environ) {
    info!("starting ping");
    let ch = env.depends.ping.take().unwrap();
    for i in 0.. {
        info!("{}: sending message", i);
        ch.send(PingRequest {
            int_value1: 42,
            bytes_value1: &[0xab, 0xcd, 0xef],
        })
        .unwrap();

        info!("{}: receiving message", i);
        let r = ch.recv_with_buffer::<PingReply>(&mut buffer).unwrap();
        info!(
            "{}: received message: {} {}",
            i,
            r.int_value2(),
            r.str_value2().to_str().unwrap()
        );

        for _ in 0..2000000 {
            unsafe {
                ::core::arch::asm!("nop");
            }
        }
    }
}
