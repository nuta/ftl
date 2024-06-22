#![no_std]
#![no_main]

use ftl_api::{channel::Channel, handle::OwnedHandle, prelude::*, types::{handle::HandleId, message::{MessageBuffer, MessageInfo}}};

#[ftl_api::main]
pub fn main() {
    println!("starting ping");
    let handle_id = HandleId::from_raw(1);
    let handle = OwnedHandle::from_raw(handle_id);
    let ch = Channel::from_handle(handle);

    loop {
        let msginfo = MessageInfo::from_raw(0);
        let message = MessageBuffer {
            handles: [HandleId::from_raw(0); 4],
            data: [0; 4096 - 4 * core::mem::size_of::<HandleId>()],
        };

        println!("sending message");
        ch.send(msginfo, &message).expect("failed to send");
        println!("receiving message");
        let ret_msginfo = ch.recv().expect("failed to recv");
        println!("received message: {:?}", ret_msginfo);
    }
}
