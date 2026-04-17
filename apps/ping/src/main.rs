#![no_std]
#![no_main]

use ftl::channel::Channel;
use ftl::channel::Message;
use ftl::channel::MessageId;
use ftl::channel::OpenOptions;
use ftl::channel::Peek;
use ftl::handle::Handleable;
use ftl::prelude::*;
use ftl::sink::Event;
use ftl::sink::Sink;

#[ftl::main]
fn main(supervisor_ch: Channel) {
    info!("starting ping");
    let sink = Sink::new().unwrap();

    sink.add(&supervisor_ch).unwrap();

    // Ask the supervisor process to connect to the pong service.
    let mid = MessageId::new(1);
    let path = b"service/pong";
    let options = OpenOptions::CONNECT;
    supervisor_ch
        .send(Message::Open {
            mid,
            path: path.as_slice(),
            options,
        })
        .unwrap();

    // Wait for the pong channel.
    let pong_ch = loop {
        let (id, event) = sink.wait().unwrap();
        match event {
            Event::Message(peeked) if id == supervisor_ch.handle().id() => {
                match Peek::parse(&supervisor_ch, peeked) {
                    Peek::OpenReply { recv } => {
                        match recv.recv() {
                            Ok(handle) => {
                                break Channel::from_handle(handle);
                            }
                            Err(error) => {
                                warn!("failed to recv with handle: {:?}", error);
                            }
                        }
                    }
                    _ => {
                        warn!("unhandled message: {:?}", peeked);
                    }
                }
            }
            _ => {
                warn!("unhandled event: {:?}", event);
            }
        }
    };

    trace!("connected to pong: {:?}", pong_ch.handle().id());
    sink.add(&pong_ch).unwrap();

    // Send the first message to the pong service.
    let mid = MessageId::new(1);
    let body = b"Hello, world!";
    pong_ch
        .send(Message::Write {
            mid,
            offset: 0,
            buf: body,
        })
        .unwrap();

    let mut num_received = 0;
    loop {
        let (id, event) = sink.wait().unwrap();
        match event {
            Event::Message(peeked) if id == pong_ch.handle().id() => {
                match Peek::parse(&pong_ch, peeked) {
                    Peek::WriteReply { recv, len } => {
                        recv.recv().unwrap();
                        info!("received write reply: written_len={}", len);

                        num_received += 1;
                        if num_received >= 10 {
                            break;
                        }

                        // Reuse the same message ID. No problem since this is an arbitrary
                        // data we can freely choose. This does not matter until we have
                        // concurrent messages anyway.
                        let mid = MessageId::new(1);

                        // Send more messages!
                        pong_ch
                            .send(Message::Write {
                                mid,
                                offset: 0,
                                buf: body,
                            })
                            .unwrap();
                    }
                    _ => {
                        warn!("unhandled message: {:?}", peeked);
                    }
                }
            }
            _ => {
                warn!("unhandled event: {:?}", event);
            }
        }
    }
}
