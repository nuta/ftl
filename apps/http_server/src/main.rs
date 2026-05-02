#![no_std]
#![no_main]

use core::ops::ControlFlow;

use ftl::channel::Channel;
use ftl::channel::Incoming;
use ftl::channel::Message;
use ftl::channel::MessageId;
use ftl::channel::OpenOptions;
use ftl::collections::HashMap;
use ftl::error::ErrorCode;
use ftl::handle::HandleId;
use ftl::handle::Handleable;
use ftl::prelude::*;
use ftl::sink::Event;
use ftl::sink::Sink;

use crate::connection::Connection;

mod connection;

const RECV_BUFFER_SIZE: usize = 4096;
const MESSAGE_ID: MessageId = MessageId::new(1);

enum Context {
    Supervisor { ch: Channel },
    Tcpip { ch: Channel },
    TcpListener { ch: Channel },
    TcpConn { ch: Channel, conn: Connection },
}

fn open_channel(
    sink: &Sink,
    ch: &Channel,
    path: &[u8],
    options: OpenOptions,
    description: &str,
) -> Channel {
    ch.send(Message::Open {
        mid: MESSAGE_ID,
        path,
        options,
    })
    .unwrap();

    loop {
        let (id, event) = sink.wait().unwrap();
        match event {
            Event::Message(peek) if id == ch.handle().id() => {
                match Incoming::parse(ch, peek) {
                    Incoming::OpenReply(reply) if reply.mid() == MESSAGE_ID => {
                        match reply.recv() {
                            Ok(handle) => return Channel::from_handle(handle),
                            Err(error) => panic!("failed to open {description}: {:?}", error),
                        }
                    }
                    Incoming::OpenReply(reply) => {
                        warn!(
                            "unexpected open reply while opening {}: mid={:?}",
                            description,
                            reply.mid()
                        );
                    }
                    _ => {
                        warn!(
                            "unhandled message while opening {}: {:?}",
                            description, peek
                        );
                    }
                }
            }
            _ => {
                warn!("unhandled event while opening {}: {:?}", description, event);
            }
        }
    }
}

fn queue_accept(listener_ch: &Channel) -> Result<(), ErrorCode> {
    listener_ch.send(Message::Open {
        mid: MESSAGE_ID,
        path: b"",
        options: OpenOptions::CONNECT,
    })
}

fn queue_read(ch: &Channel) -> Result<(), ErrorCode> {
    ch.send(Message::Read {
        mid: MESSAGE_ID,
        offset: 0,
        len: RECV_BUFFER_SIZE,
    })
}

fn queue_next_write(ch: &Channel, conn: &mut Connection) -> bool {
    let Some(data) = conn.poll_send() else {
        return false;
    };

    if let Err(error) = ch.send(Message::Write {
        mid: MESSAGE_ID,
        offset: 0,
        buf: &data[..],
    }) {
        warn!("failed to send write message: {:?}", error);
        return false;
    }

    true
}

fn remove_from_sink(sink: &Sink, id: HandleId) {
    if let Err(error) = sink.remove(id) {
        warn!("failed to remove handle from sink: {:?}", error);
    }
}

fn handle_listener_message(
    sink: &Sink,
    contexts: &mut HashMap<HandleId, Context>,
    listener_ch: &Channel,
    incoming: Incoming<&Channel>,
) {
    match incoming {
        Incoming::OpenReply(reply) => {
            trace!("accepted a connection");

            if reply.mid() != MESSAGE_ID {
                warn!("unexpected accept reply: mid={:?}", reply.mid());
                return;
            }

            let conn_ch = match reply.recv() {
                Ok(handle) => Channel::from_handle(handle),
                Err(error) => {
                    warn!("failed to receive accepted connection: {:?}", error);
                    return;
                }
            };

            if let Err(error) = queue_accept(listener_ch) {
                warn!("failed to queue accept: {:?}", error);
            }

            let conn_id = conn_ch.handle().id();
            if let Err(error) = sink.add(&conn_ch) {
                warn!("failed to add connection to sink: {:?}", error);
                return;
            }

            if let Err(error) = queue_read(&conn_ch) {
                warn!("failed to send read message: {:?}", error);
                remove_from_sink(sink, conn_id);
                return;
            }

            contexts.insert(
                conn_id,
                Context::TcpConn {
                    ch: conn_ch,
                    conn: Connection::new(),
                },
            );
        }
        _ => {
            warn!("unhandled listener message");
        }
    }
}

fn handle_connection_message(
    ch: &Channel,
    conn: &mut Connection,
    incoming: Incoming<&Channel>,
) -> bool {
    match incoming {
        Incoming::ReadReply(reply) => {
            let len = reply.read_len();
            let mut buf = vec![0; len];
            let data = match reply.recv(&mut buf) {
                Ok(data) => data,
                Err(error) => {
                    warn!("failed to recv read body: {:?}", error);
                    return false;
                }
            };

            trace!("received {} bytes", data.len());
            if matches!(conn.handle_recv(data), ControlFlow::Continue(())) {
                if let Err(error) = queue_read(ch) {
                    warn!("failed to send read message: {:?}", error);
                    return false;
                }

                true
            } else {
                queue_next_write(ch, conn)
            }
        }
        Incoming::WriteReply(reply) => {
            trace!("wrote {} bytes", reply.written_len());
            drop(reply);
            queue_next_write(ch, conn)
        }
        Incoming::ErrorReply(reply) => {
            warn!("connection error: {:?}", reply.error());
            false
        }
        _ => {
            warn!("unhandled connection message");
            true
        }
    }
}

#[ftl::main]
fn main(supervisor_ch: Channel) {
    info!("starting http_server");

    let sink = Sink::new().unwrap();
    sink.add(&supervisor_ch).unwrap();

    let tcpip_ch = open_channel(
        &sink,
        &supervisor_ch,
        b"service/tcpip",
        OpenOptions::CONNECT,
        "tcpip service",
    );
    sink.add(&tcpip_ch).unwrap();

    let listener_ch = open_channel(
        &sink,
        &tcpip_ch,
        b"tcp-listen:0.0.0.0:80",
        OpenOptions::LISTEN,
        "tcp listener",
    );
    sink.add(&listener_ch).unwrap();

    if let Err(error) = queue_accept(&listener_ch) {
        warn!("failed to queue accept: {:?}", error);
    }

    trace!("listening on 80");

    let mut contexts = HashMap::new();
    contexts.insert(
        supervisor_ch.handle().id(),
        Context::Supervisor { ch: supervisor_ch },
    );
    contexts.insert(tcpip_ch.handle().id(), Context::Tcpip { ch: tcpip_ch });
    contexts.insert(
        listener_ch.handle().id(),
        Context::TcpListener { ch: listener_ch },
    );

    loop {
        let (id, event) = sink.wait().unwrap();
        let Some(mut context) = contexts.remove(&id) else {
            warn!("event for unknown handle {:?}: {:?}", id, event);
            continue;
        };

        let keep = match (&mut context, event) {
            (Context::Supervisor { ch }, Event::Message(peek)) => {
                match Incoming::parse(&*ch, peek) {
                    _ => warn!("unhandled supervisor message: {:?}", peek),
                }
                true
            }
            (Context::Tcpip { ch }, Event::Message(peek)) => {
                match Incoming::parse(&*ch, peek) {
                    _ => warn!("unhandled tcpip message: {:?}", peek),
                }
                true
            }
            (Context::TcpListener { ch }, Event::Message(peek)) => {
                let incoming = Incoming::parse(&*ch, peek);
                handle_listener_message(&sink, &mut contexts, ch, incoming);
                true
            }
            (Context::TcpConn { ch, conn }, Event::Message(peek)) => {
                let incoming = Incoming::parse(&*ch, peek);
                handle_connection_message(ch, conn, incoming)
            }
            (_, Event::PeerClosed) => {
                trace!("peer closed: {:?}", id);
                false
            }
            (_, event) => {
                warn!("unhandled event: {:?}", event);
                true
            }
        };

        if keep {
            contexts.insert(id, context);
        } else {
            remove_from_sink(&sink, id);
        }
    }
}
