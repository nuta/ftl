#![no_std]
#![no_main]

use ftl_api::{channel::Channel, handle::OwnedHandle, message::MessageBuffer, prelude::*, types::{handle::HandleId, message::MessageInfo}};

#[ftl_api::main]
pub fn main() {
    println!("[pong] starting pong");
    let handle_id = HandleId::from_raw(1);
    let handle = OwnedHandle::from_raw(handle_id);
    let ch = Channel::from_handle(handle);

    let mut message = MessageBuffer {
        data: [0; 4095],
    };

    for i in 0.. {
        println!("[pong] receiving message");
        let ret_msginfo = ch.recv(&mut message).expect("failed to recv");
        println!("[pong] received message: {:x?} \"{}\"", ret_msginfo, core::str::from_utf8(&message.data[0..5]).unwrap());

        println!("[pong] replying message 2");
        let msginfo = MessageInfo::from_raw(i << 20 | 5);
        message.data[0..5].copy_from_slice("WORLD".as_bytes());
        println!("sending ....: {:?}", msginfo);
        ch.send(msginfo, &message).expect("failed to send");
    }
}
