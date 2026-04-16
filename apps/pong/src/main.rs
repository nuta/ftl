#![no_std]
#![no_main]

use ftl::channel::Channel;
use ftl::channel::Message;
use ftl::channel::MessageId;
use ftl::channel::MessageKind;
use ftl::channel::OpenOptions;
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
            Event::Message { info, .. } if id == supervisor_ch.handle().id() => {
                match info.kind() {
                    MessageKind::OPEN_REPLY => {
                        let handle = supervisor_ch.recv_handle(info).unwrap();
                        break Channel::from_handle(handle);
                    }
                    _ => {
                        warn!("unhandled message: {:?}", info.kind());
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
            (Context::Server, Event::Message { info, .. }) => {
                match info.kind() {
                    MessageKind::OPEN => {
                        if info.body_len() > 1024 {
                            server_ch
                                .send(Message::ErrorReply {
                                    mid: info.mid(),
                                    error: ErrorCode::InvalidArgument,
                                }).unwrap();
                            continue;
                        }

                        // Receive the message.
                        let mut buf = vec![0; info.body_len()];
                        if let Err(error) = server_ch.recv_body(info, &mut buf) {
                            warn!("failed to recv with body: {:?}", error);
                            continue;
                        }

                        let (our_ch, their_ch) = Channel::new().unwrap();
                        sink.add(&our_ch).unwrap();
                        contexts.insert(our_ch.handle().id(), Context::Client { ch: our_ch });

                        // Reply to the client.
                        server_ch
                            .send(Message::OpenReply {
                                mid: info.mid(),
                                handle: their_ch.into_handle(),
                            })
                            .unwrap();

                        info!("accepted a client");
                    }
                    _ => {
                        warn!("unhandled message: {:?}", info.kind());
                    }
                }
            }
            (Context::Client { ch }, Event::Message { info, .. }) => {
                match info.kind() {
                    MessageKind::WRITE => {
                        if info.body_len() > 1024 {
                            ch.send(Message::ErrorReply {
                                mid: info.mid(),
                                error: ErrorCode::InvalidArgument,
                            }).unwrap();
                            continue;
                        }

                        let mut buf = vec![0; info.body_len()];
                        if let Err(error) = ch.recv_body(info, &mut buf) {
                            warn!("failed to recv with body: {:?}", error);
                            continue;
                        }

                        info!("received write message: {:?}", core::str::from_utf8(&buf));
                        ch.send(Message::WriteReply {
                            mid: info.mid(),
                            len: buf.len(),
                        }).unwrap();
                    }
                    _ => {
                        warn!("unhandled message: {:?}", info.kind());
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
