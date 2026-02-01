#![no_std]
#![no_main]

use ftl::channel::Channel;
use ftl::channel::Reply;
use ftl::handle::HandleId;
use ftl::handle::OwnedHandle;
use ftl::println;
use ftl::sink::Event;
use ftl::sink::Sink;

#[unsafe(no_mangle)]
fn main() {
    println!("[pong] started");
    let ch_id = HandleId::from_raw(1);
    let ch = Channel::from_handle(OwnedHandle::from_raw(ch_id));

    let sink = Sink::new().unwrap();
    sink.add(&ch).unwrap();
    loop {
        let event = sink.wait().unwrap();
        match event {
            Event::CallMessage {
                info,
                call_id,
                handles,
                inline,
            } => {
                println!("[pong] received call message: {:?}", info);
                ch.reply(call_id, Reply::WriteReply { len: 13 }).unwrap();
            }
            _ => {
                panic!("[pong] received unexpected event");
            }
        }
    }
}
