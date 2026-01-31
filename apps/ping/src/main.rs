#![no_std]
#![no_main]

use ftl::channel::Buffer;
use ftl::channel::Channel;
use ftl::channel::Message;
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
    println!("[ping] starting");

    // Get the channel at handle ID 1 (pre-populated by the loader).
    let channel = unsafe { Channel::from_raw_id(HandleId::from_raw(1)) };

    // Create a sink and add the channel to it.
    let sink = Sink::new().expect("failed to create sink");
    sink.add(&channel).expect("failed to add channel to sink");

    let mut counter = 0u64;
    loop {
        counter += 1;

        // Send a Write message to pong.
        println!("[ping #{}] sending", counter);
        channel
            .send(Message::Write {
                offset: 0,
                data: Buffer::Static(b"Hello from ping!"),
            })
            .expect("failed to send message");

        // Wait for the reply.
        let msg = sink.wait().expect("failed to receive message");

        if msg.info == MessageInfo::WRITE_REPLY {
            println!("[ping #{}] got reply", counter);
        } else {
            println!("[ping #{}] unexpected: kind={}", counter, msg.info.kind());
        }

        busy_wait();
    }
}
