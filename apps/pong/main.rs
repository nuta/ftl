#![no_std]
#![no_main]

use ftl_api::{channel::Channel, handle::OwnedHandle, prelude::*, types::{handle::HandleId, message::{MessageBuffer, MessageInfo}}};

#[ftl_api::main]
pub fn main() {
    println!("[pong] starting pong");
    let handle_id = HandleId::from_raw(1);
    let handle = OwnedHandle::from_raw(handle_id);
    let ch = Channel::from_handle(handle);

    let mut message = MessageBuffer {
        handles: [HandleId::from_raw(0); 4],
        data: [0; 4096 - 4 * core::mem::size_of::<HandleId>()],
    };

    for i in 0.. {
        println!("[pong] receiving message");
        let ret_msginfo = ch.recv(&mut message).expect("failed to recv");
        println!("[pong] received message: {:x?}", ret_msginfo);

        println!("[pong] replying message 2");
        let msginfo = MessageInfo::from_raw(i << 20);
        println!("sending ....: {:?}", msginfo);
        ch.send(msginfo, &message).expect("failed to send");
    }
}