#![no_std]
#![no_main]

use core::ops::ControlFlow;

use ftl::channel::Buffer;
use ftl::channel::BufferMut;
use ftl::channel::Channel;
use ftl::collections::HashMap;
use ftl::error::ErrorCode;
use ftl::eventloop::Client;
use ftl::eventloop::Event;
use ftl::eventloop::EventLoop;
use ftl::handle::HandleId;
use ftl::handle::Handleable;
use ftl::log::*;
use ftl::prelude::vec;
use ftl::rc::Rc;

use crate::connection::Connection;

mod connection;

const RECV_BUFFER_SIZE: usize = 4096;

#[derive(Debug)]
enum Cookie {
    OpenListen,
    OpenConn,
    Read,
    Write,
}

#[derive(Debug)]
enum Context {
    Tcpip,
    TcpListener,
    TcpConn(Connection),
}

#[ftl::main]
fn main() {
    let mut eventloop = EventLoop::new().unwrap();

    let tcpip_ch = Rc::new(Channel::connect("tcpip").unwrap());
    let tcpip_client = eventloop.add_channel(tcpip_ch, Context::Tcpip).unwrap();

    tcpip_client
        .open("tcp-listen:0.0.0.0:80", Cookie::OpenListen)
        .expect("failed to send open message");

    loop {
        match eventloop.wait() {
            Event::OpenReply {
                ctx: Context::Tcpip,
                cookie: Cookie::OpenListen,
                new_ch,
                ..
            } => {
                trace!("listening on 80");
                tcpip_client
                    .open("", Cookie::OpenConn)
                    .expect("failed to send open message");
            }
            Event::OpenReply {
                ctx: Context::Tcpip,
                cookie: Cookie::OpenConn,
                new_ch,
                ..
            } => {
                trace!("accepted a connection");
                let conn = Connection::new();
                let client = match eventloop.add_channel(new_ch, Context::TcpConn(conn)) {
                    Ok(client) => client,
                    Err(err) => {
                        warn!("failed to add connection to event loop: {:?}", err);
                        return;
                    }
                };

                // Read the first chunk of data asynchronously.
                client
                    .read(0, vec![0; RECV_BUFFER_SIZE], Cookie::Read)
                    .expect("failed to send read message");
            }
            Event::ReadReply {
                ctx: Context::TcpConn(conn),
                cookie: Cookie::Read,
                client,
                buf,
                len,
                ..
            } => {
                trace!("received {} bytes", len);

                let BufferMut::Vec(mut buf) = buf else {
                    unreachable!()
                };

                buf.truncate(len);
                if matches!(conn.handle_recv(buf), ControlFlow::Continue(())) {
                    client
                        .read(0, vec![0; RECV_BUFFER_SIZE], Cookie::Read)
                        .expect("failed to send read message");
                } else {
                    if let Some(data) = conn.poll_send() {
                        if let Err(error) = client.write(0, data, Cookie::Write) {
                            warn!("failed to send write message: {:?}", error);
                            // TODO: Close channel
                        }
                    } else {
                        // TODO: Close channel
                    }
                }
            }
            Event::WriteReply {
                ctx: Context::TcpConn(conn),
                cookie: Cookie::Write,
                client,
                buf,
                len,
                ..
            } => {
                if let Some(data) = conn.poll_send() {
                    if let Err(error) = client.write(0, data, Cookie::Write) {
                        warn!("failed to send write message: {:?}", error);
                        // TODO: Close channel
                    }
                } else {
                    // TODO: Close channel
                }
            }
            Event::ErrorReply { client, error, .. } => {
                warn!("error reply from {:?}", error);
                // TODO: Close channel
            }
            Event::PeerClosed { ch, .. } => {
                trace!("peer closed: {:?}", ch);
                // TODO: Close channel
                let id = ch.handle().id();
                eventloop.remove(id).unwrap();
            }
            event => {
                warn!("unhandled event: {:?}", event);
            }
        }
    }
}
