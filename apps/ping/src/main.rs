#![no_std]
#![no_main]

use ftl::channel::Buffer;
use ftl::channel::Channel;
use ftl::channel::Message;
use ftl::handle::HandleId;
use ftl::handle::OwnedHandle;
use ftl::println;
use ftl::sink::Event;
use ftl::sink::Sink;

#[unsafe(no_mangle)]
fn main() {
    println!("[ping] started");
    let ch_id = HandleId::from_raw(1);
    let ch = Channel::from_handle(OwnedHandle::from_raw(ch_id));

    let sink = Sink::new().unwrap();
    sink.add(&ch).unwrap();
    loop {
        println!("[ping] sending message");
        ch.send(Message::Write {
            offset: 0,
            data: Buffer::Static(b"Hello, world!"),
        })
        .unwrap();

        let event = sink.wait().unwrap();
        match event {
            Event::ReplyMessage {
                info,
                cookie,
                handles,
                inline,
            } => {
                println!("[ping] received reply message: {:?}", info);
            }
            _ => {
                panic!("[ping] received unexpected event");
            }
        }
    }
}
