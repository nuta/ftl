#![no_std]
#![no_main]

use ftl_api::{channel::Channel, handle::OwnedHandle, message::MessageBuffer, prelude::*, types::{handle::HandleId, message::MessageInfo}};

#[ftl_api::main]
pub fn main() {
    println!("[ping] starting ping");
    let handle_id = HandleId::from_raw(1);
    let handle = OwnedHandle::from_raw(handle_id);
    let ch = Channel::from_handle(handle);

    let mut message = MessageBuffer {
        data: [0; 4095],
    };

    for i in 0.. {
        println!("[ping] sending message");
        message.data[0..5].copy_from_slice("HELLO".as_bytes());
        let msginfo = MessageInfo::from_raw(i << 20 | 5);
        ch.send(msginfo, &message).expect("failed to send");

        println!("[ping] receiving message");
        let ret_msginfo = ch.recv(&mut message).expect("failed to recv");
        println!("[ping] received message: {:x?} \"{}\"", ret_msginfo, core::str::from_utf8(&message.data[0..5]).unwrap());
    }
}
