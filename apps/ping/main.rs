#![no_std]
#![no_main]

use ftl_api::{channel::Channel, handle::OwnedHandle, prelude::*, types::{handle::HandleId, message::{MessageBuffer, MessageInfo}}};

#[ftl_api::main]
pub fn main() {
    println!("[ping] starting ping");
    let handle_id = HandleId::from_raw(1);
    let handle = OwnedHandle::from_raw(handle_id);
    let ch = Channel::from_handle(handle);

    let mut message = MessageBuffer {
        handles: [HandleId::from_raw(0); 4],
        data: [0; 4096 - 4 * core::mem::size_of::<HandleId>()],
    };

    loop {
        println!("[ping] sending message");
        let msginfo = MessageInfo::from_raw(0);
        ch.send(msginfo, &message).expect("failed to send");

        println!("[ping] receiving message");
        let ret_msginfo = ch.recv(&mut message).expect("failed to recv");
        println!("[ping] received message: {:?}", ret_msginfo);
    }
}
