#![no_std]
#![no_main]

use ftl::channel::Channel;
use ftl::channel::MessageId;
use ftl::channel::MessageInfo;
use ftl::channel::MessageKind;
use ftl::channel::OpenOptions;
use ftl::handle::Handleable;
use ftl::prelude::*;
use ftl::sink::Event;
use ftl::sink::Sink;

#[ftl::main]
fn main() {
    info!("starting ping");
    let sink = Sink::new().unwrap();

    use ftl::handle::HandleId;
    use ftl::handle::OwnedHandle;
    let supervisor_ch = Channel::from_handle(OwnedHandle::from_raw(HandleId::from_raw(1)));
    sink.add(&supervisor_ch).unwrap();

    // Ask the supervisor process to connect to the pong service.
    let mid = MessageId::new(1);
    let path = b"service/pong";
    let info = MessageInfo::new(MessageKind::OPEN, mid, path.len());
    let options = OpenOptions::OPEN;
    supervisor_ch
        .send_with_body(info, options.as_usize(), path)
        .unwrap();

    // Wait for the pong channel.
    let pong_ch = loop {
        let (id, event) = sink.wait().unwrap();
        match event {
            Event::Message { info, arg1, arg2 } if id == supervisor_ch.handle().id() => {
                match info.kind() {
                    MessageKind::OPEN_REPLY => {
                        let handle = supervisor_ch.recv_with_handle(info).unwrap();
                        break Channel::from_handle(handle);
                    }
                    kind => {
                        warn!("unhandled message: {:?}", kind);
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
    let info = MessageInfo::new(MessageKind::WRITE, mid, body.len());
    pong_ch.send_with_body(info, 0, body).unwrap();

    let mut num_received = 0;
    loop {
        let (id, event) = sink.wait().unwrap();
        match event {
            Event::Message { info, arg1, arg2 } if id == pong_ch.handle().id() => {
                match info.kind() {
                    MessageKind::WRITE_REPLY => {
                        pong_ch.recv(info).unwrap();
                        info!("received write reply: written_len={arg1}");

                        num_received += 1;
                        if num_received >= 10 {
                            break;
                        }

                        // Reuse the same message ID. No problem since this is an arbitrary
                        // data we can freely choose. This does not matter until we have
                        // concurrent messages anyway.
                        let mid = MessageId::new(1);

                        // Send more messages!
                        let info = MessageInfo::new(MessageKind::WRITE, mid, body.len());
                        pong_ch.send_with_body(info, 0, body).unwrap();
                    }
                    kind => {
                        warn!("unhandled message: {:?}", kind);
                    }
                }
            }
            _ => {
                warn!("unhandled event: {:?}", event);
            }
        }
    }
}
