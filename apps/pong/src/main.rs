#![no_std]
#![no_main]

use ftl::channel::Channel;
use ftl::channel::Reply;
use ftl::println;
use ftl::sink::Sink;
use ftl_types::channel::MessageInfo;
use ftl_types::handle::HandleId;

fn busy_wait() {
    for _ in 0..1_000_000 {
        unsafe { core::arch::asm!("nop") }
    }
}

#[unsafe(no_mangle)]
fn main() {
    println!("[pong] starting");

    // Get the channel at handle ID 1 (pre-populated by the loader).
    let channel = unsafe { Channel::from_raw_id(HandleId::from_raw(1)) };

    // Create a sink and add the channel to it.
    let sink = Sink::new().expect("failed to create sink");
    sink.add(&channel).expect("failed to add channel to sink");

    let mut counter = 0u64;
    loop {
        let msg = sink.wait().expect("failed to receive message");
        counter += 1;

        if msg.info == MessageInfo::WRITE {
            println!("[pong #{}] got request, replying", counter);
            channel
                .reply(msg.call_id, Reply::WriteReply { len: 17 })
                .expect("failed to send reply");
        } else {
            println!("[pong #{}] unexpected: kind={}", counter, msg.info.kind());
        }

        busy_wait();
    }
}
