#![no_std]
#![no_main]

use core::ops::ControlFlow;

use ftl::application::Application;
use ftl::application::Context;
use ftl::channel::Buffer;
use ftl::channel::BufferMut;
use ftl::channel::Channel;
use ftl::channel::Message;
use ftl::collections::HashMap;
use ftl::handle::HandleId;
use ftl::handle::Handleable;
use ftl::handle::OwnedHandle;
use ftl::prelude::vec;
use ftl::println;
use ftl::rc::Rc;

use crate::connection::Connection;

mod connection;

const RECV_BUFFER_SIZE: usize = 4096;

enum State {
    Tcpip,
    TcpListener,
    TcpConn(Connection),
}

struct Main {
    states: HashMap<HandleId, State>,
}

fn open_tcpip_ch() -> Rc<Channel> {
    let control_id = HandleId::from_raw(1);
    Rc::new(Channel::from_handle(OwnedHandle::from_raw(control_id)))
}

impl Application for Main {
    fn init(ctx: &mut Context) -> Self {
        let mut states = HashMap::new();

        let tcpip = open_tcpip_ch();
        ctx.add_channel(tcpip.clone()).unwrap();
        states.insert(tcpip.handle().id(), State::Tcpip);

        tcpip
            .send(Message::Open {
                uri: Buffer::Static(b"tcp-listen:0.0.0.0:80"),
            })
            .expect("failed to send open message");

        Self { states }
    }

    fn open_reply(&mut self, ctx: &mut Context, ch: &Rc<Channel>, uri: Buffer, new_ch: Channel) {
        match self.states.get_mut(&ch.handle().id()) {
            Some(State::Tcpip) => {
                // Request to accept a new connection.
                new_ch
                    .send(Message::Open {
                        uri: Buffer::Static(b""),
                    })
                    .expect("failed to send accept message");

                let listen_ch_id = new_ch.handle().id();
                ctx.add_channel(Rc::new(new_ch)).unwrap();
                self.states.insert(listen_ch_id, State::TcpListener);
            }
            Some(State::TcpListener) => {
                // Accepted a new connection.
                let conn_ch_id = new_ch.handle().id();

                // Provide a read buffer to TCP/IP server.
                new_ch
                    .send(Message::Read {
                        offset: 0,
                        data: BufferMut::Vec(vec![0; RECV_BUFFER_SIZE]),
                    })
                    .expect("failed to send read message");

                let conn = Connection::new();
                ctx.add_channel(Rc::new(new_ch)).unwrap();
                self.states.insert(conn_ch_id, State::TcpConn(conn));
            }
            _ => {
                println!("unexpected open reply on {:?}", ch.handle().id());
            }
        }
    }

    fn read_reply(&mut self, ctx: &mut Context, ch: &Rc<Channel>, buf: BufferMut, len: usize) {
        match self.states.get_mut(&ch.handle().id()) {
            Some(State::TcpConn(conn)) => {
                println!("received {} bytes from {:?}", len, ch.handle().id());
                let BufferMut::Vec(mut buf) = buf else {
                    unreachable!()
                };

                buf.truncate(len);
                match conn.handle_recv(buf) {
                    ControlFlow::Continue(()) => {
                        // Keep reading more data.
                        ch.send(Message::Read {
                            offset: 0,
                            data: BufferMut::Vec(vec![0; RECV_BUFFER_SIZE]),
                        })
                        .expect("failed to send read message");
                    }
                    ControlFlow::Break(()) => {
                        // No more data to read.
                    }
                }

                if let Some(message) = conn.poll_send() {
                    ch.send(message).expect("failed to send write message");
                }
            }
            _ => {
                println!("unexpected read reply on {:?}", ch.handle().id());
            }
        }
    }

    fn write_reply(&mut self, ctx: &mut Context, ch: &Rc<Channel>, buf: Buffer, len: usize) {
        match self.states.get_mut(&ch.handle().id()) {
            Some(State::TcpConn(conn)) => {
                if let Some(message) = conn.poll_send() {
                    ch.send(message).expect("failed to send write message");
                }
            }
            _ => {
                println!("unexpected write reply on {:?}", ch.handle().id());
            }
        }
    }
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::run::<Main>();
}
