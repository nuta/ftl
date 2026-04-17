#![no_std]
#![no_main]

use ftl::channel::Channel;
use ftl::channel::Message;
use ftl::channel::MessageId;
use ftl::channel::MessageKind;
use ftl::channel::OpenOptions;
use ftl::channel::Peek;
use ftl::collections::HashMap;
use ftl::error::ErrorCode;
use ftl::handle::Handleable;
use ftl::prelude::*;
use ftl::sink::Event;
use ftl::sink::Sink;

enum Context {
    Server,
    Client { ch: Channel },
}

#[ftl::main]
fn main(supervisor_ch: Channel) {
    info!("starting pong");
    let sink = Sink::new().unwrap();

    sink.add(&supervisor_ch).unwrap();

    // Ask the supervisor process to register this service.
    let listen_mid = MessageId::new(1);
    let path = b"service/pong";
    let options = OpenOptions::LISTEN;
    supervisor_ch
        .send(Message::Open {
            mid: listen_mid,
            path: path.as_slice(),
            options,
        })
        .unwrap();

    // Wait for the supervisor process to register this service.
    let server_ch = loop {
        let (id, event) = sink.wait().unwrap();
        match event {
            Event::Message(peeked) if id == supervisor_ch.handle().id() => {
                match Peek::parse(&supervisor_ch, peeked) {
                    Peek::OpenReply { recv } => {
                        let handle = recv.recv().unwrap();
                        break Channel::from_handle(handle);
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

    // Wait for clients to connect.
    sink.add(&server_ch).unwrap();
    let mut contexts = HashMap::new();
    contexts.insert(server_ch.handle().id(), Context::Server);
    loop {
        let (id, event) = sink.wait().unwrap();
        let context = contexts.get(&id).unwrap();
        match (context, event) {
            (Context::Server, Event::Message(peeked)) => {
                match Peek::parse(&server_ch, peeked) {
                    Peek::Open {
                        recv,
                        options,
                        path_len,
                        completer,
                    } => {
                        if path_len > 1024 {
                            completer.reply_error(ErrorCode::InvalidArgument);
                            continue;
                        }

                        // Receive the message.
                        let mut buf = vec![0; path_len];
                        if let Err(error) = recv.recv(&mut buf) {
                            warn!("failed to recv with body: {:?}", error);
                            continue;
                        }

                        let (our_ch, their_ch) = Channel::new().unwrap();
                        sink.add(&our_ch).unwrap();
                        contexts.insert(our_ch.handle().id(), Context::Client { ch: our_ch });

                        // Reply to the client.
                        completer.reply(their_ch.into_handle());

                        info!("accepted a client");
                    }
                    _ => {
                        warn!("unhandled message: {:?}", peeked);
                    }
                }
            }
            (Context::Client { ch }, Event::Message(peeked)) => {
                match Peek::parse(ch, peeked) {
                    Peek::Write {
                        recv,
                        len,
                        completer,
                    } => {
                        if len > 1024 {
                            completer.reply_error(ErrorCode::InvalidArgument);
                            continue;
                        }

                        let mut buf = vec![0; len];
                        if let Err(error) = recv.recv(&mut buf) {
                            warn!("failed to recv with body: {:?}", error);
                            continue;
                        }

                        info!("received write message: {:?}", core::str::from_utf8(&buf));
                        completer.reply(buf.len());
                    }
                    _ => {
                        warn!("unhandled message: {:?}", peeked);
                    }
                }
            }
            (_, Event::PeerClosed) => {
                sink.remove(id).unwrap();
                contexts.remove(&id);
            }
        }
    }
}
