#![no_std]
#![no_main]

use core::ops::ControlFlow;

use ftl::buffer::Buffer;
use ftl::buffer::BufferUninit;
use ftl::channel::Channel;
use ftl::eventloop::Event;
use ftl::eventloop::EventLoop;
use ftl::eventloop::Reply;
use ftl::handle::Handleable;
use ftl::log::*;

use crate::connection::Connection;

mod connection;

const RECV_BUFFER_SIZE: usize = 4096;

#[derive(Debug)]
enum Cookie {
    ConnectTcpip,
    OpenListen,
    OpenConn,
    Read,
    Write,
}

#[derive(Debug)]
enum Context {
    Bootstrap,
    Tcpip,
    TcpListener,
    TcpConn(Connection),
}

#[ftl::main]
fn main() {
    let mut eventloop = EventLoop::new().unwrap();

    // FIXME:
    use ftl::handle::HandleId;
    use ftl::handle::OwnedHandle;
    let bootstrap_ch = Channel::from_handle(OwnedHandle::from_raw(HandleId::from_raw(1)));

    let bootstrap_client = eventloop
        .add_channel(bootstrap_ch, Context::Bootstrap)
        .unwrap();

    bootstrap_client
        .open("connect:tcpip", Cookie::ConnectTcpip)
        .unwrap();
    let tcpip_ch = loop {
        match eventloop.wait() {
            Event::Reply {
                ctx: Context::Bootstrap,
                reply:
                    Reply::Open {
                        cookie: Cookie::ConnectTcpip,
                        new_ch,
                        ..
                    },
            } => {
                break new_ch;
            }
            event => {
                panic!("unexpected bootstrap event: {:?}", event);
            }
        }
    };

    let tcpip_client = eventloop.add_channel(tcpip_ch, Context::Tcpip).unwrap();

    tcpip_client
        .open("tcp-listen:0.0.0.0:80", Cookie::OpenListen)
        .expect("failed to send open message");

    loop {
        match eventloop.wait() {
            Event::Reply {
                ctx: Context::Tcpip,
                reply:
                    Reply::Open {
                        cookie: Cookie::OpenListen,
                        new_ch,
                        ..
                    },
            } => {
                trace!("listening on 80");
                let listener = match eventloop.add_channel(new_ch, Context::TcpListener) {
                    Ok(client) => client,
                    Err(err) => {
                        warn!("failed to add listener to event loop: {:?}", err);
                        return;
                    }
                };

                // Accept the first connection asynchronously.
                if let Err(error) = listener.open("", Cookie::OpenConn) {
                    warn!("failed to queue accept: {:?}", error);
                }
            }
            Event::Reply {
                ctx: Context::TcpListener,
                reply:
                    Reply::Open {
                        client: listener,
                        cookie: Cookie::OpenConn,
                        new_ch,
                        ..
                    },
            } => {
                trace!("accepted a connection");
                if let Err(error) = listener.open("", Cookie::OpenConn) {
                    warn!("failed to queue accept: {:?}", error);
                }

                // Add the connection to the event loop.
                let conn = Connection::new();
                let client = match eventloop.add_channel(new_ch, Context::TcpConn(conn)) {
                    Ok(client) => client,
                    Err(err) => {
                        warn!("failed to add connection to event loop: {:?}", err);
                        continue;
                    }
                };

                // Read the first chunk of data asynchronously.
                let uninit = BufferUninit::with_capacity(RECV_BUFFER_SIZE);
                if let Err(error) = client.read(0, uninit, Cookie::Read) {
                    warn!("failed to send read message: {:?}", error);
                    let id = client.channel().handle().id();
                    eventloop.remove(id);
                }
            }
            Event::Reply {
                ctx: Context::TcpConn(conn),
                reply:
                    Reply::Read {
                        cookie: Cookie::Read,
                        client,
                        buf,
                        len,
                    },
            } => {
                trace!("received {} bytes", len);

                let data: Buffer = buf.into();
                if matches!(conn.handle_recv(&data), ControlFlow::Continue(())) {
                    let uninit = BufferUninit::with_capacity(RECV_BUFFER_SIZE);
                    if let Err(error) = client.read(0, uninit, Cookie::Read) {
                        warn!("failed to send read message: {:?}", error);
                        let id = client.channel().handle().id();
                        eventloop.remove(id);
                    }
                } else {
                    if let Some(data) = conn.poll_send() {
                        if let Err(error) = client.write(0, data, Cookie::Write) {
                            warn!("failed to send write message: {:?}", error);
                            let id = client.channel().handle().id();
                            eventloop.remove(id);
                        }
                    } else {
                        // No more data to send.
                        let id = client.channel().handle().id();
                        eventloop.remove(id);
                    }
                }
            }
            Event::Reply {
                ctx: Context::TcpConn(conn),
                reply:
                    Reply::Write {
                        cookie: Cookie::Write,
                        client,
                        ..
                    },
            } => {
                if let Some(data) = conn.poll_send() {
                    if let Err(error) = client.write(0, data, Cookie::Write) {
                        warn!("failed to send write message: {:?}", error);
                        let id = client.channel().handle().id();
                        eventloop.remove(id);
                    }
                } else {
                    let id = client.channel().handle().id();
                    eventloop.remove(id);
                }
            }
            Event::Reply {
                ctx: Context::TcpConn(_),
                reply: Reply::Error { client, error, .. },
            } => {
                warn!("error reply from {:?}: {:?}", client.channel(), error);
                let id = client.channel().handle().id();
                eventloop.remove(id);
            }
            Event::Reply {
                reply: Reply::Error { client, error, .. },
                ..
            } => {
                warn!("error reply from {:?}: {:?}", client.channel(), error);
            }
            Event::PeerClosed {
                ctx: Context::TcpConn(_),
                ch,
            } => {
                trace!("peer closed: {:?}", ch);
                let id = ch.handle().id();
                eventloop.remove(id);
            }
            Event::PeerClosed { ch, .. } => {
                trace!("peer closed: {:?}", ch);
            }
            event => {
                warn!("unhandled event: {:?}", event);
            }
        }
    }
}
