#![no_std]
#![no_main]

use core::ops::ControlFlow;

use ftl::channel::Buffer;
use ftl::channel::BufferMut;
use ftl::channel::Channel;
use ftl::channel::Message;
use ftl::collections::HashMap;
use ftl::error::ErrorCode;
use ftl::eventloop::Event;
use ftl::eventloop::EventLoop;
use ftl::eventloop::ReplyEvent;
use ftl::eventloop::Request;
use ftl::handle::HandleId;
use ftl::handle::Handleable;
use ftl::log::*;
use ftl::prelude::vec;
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

impl Main {
    fn new(eventloop: &mut EventLoop) -> Self {
        let mut states = HashMap::new();

        let tcpip = Rc::new(Channel::connect("tcpip").unwrap());
        states.insert(tcpip.handle().id(), State::Tcpip);
        eventloop.add_channel(tcpip.clone()).unwrap();

        tcpip
            .send(Message::Open {
                uri: Buffer::Static(b"tcp-listen:0.0.0.0:80"),
            })
            .expect("failed to send open message");

        Self { states }
    }

    fn close_channel(&mut self, eventloop: &mut EventLoop, handle_id: HandleId) {
        if self.states.remove(&handle_id).is_some() {
            if let Err(error) = eventloop.remove(handle_id) {
                warn!(
                    "failed to remove {:?} from event loop: {:?}",
                    handle_id, error
                );
            }
        }
    }

    fn send_next_or_close(&mut self, eventloop: &mut EventLoop, ch: &Rc<Channel>) {
        let handle_id = ch.handle().id();
        match self.states.get_mut(&handle_id) {
            Some(State::TcpConn(conn)) => {
                if let Some(message) = conn.poll_send() {
                    ch.send(message).expect("failed to send write message");
                } else {
                    trace!("closing connection on {:?}", handle_id);
                    self.close_channel(eventloop, handle_id);
                }
            }
            _ => {
                trace!("unexpected connection reply on {:?}", handle_id);
            }
        }
    }

    fn on_open_reply(&mut self, eventloop: &mut EventLoop, ch: &Rc<Channel>, new_ch: Channel) {
        match self.states.get_mut(&ch.handle().id()) {
            Some(State::Tcpip) => {
                new_ch
                    .send(Message::Open {
                        uri: Buffer::Static(b""),
                    })
                    .expect("failed to send accept message");

                let listen_ch_id = new_ch.handle().id();
                eventloop.add_channel(Rc::new(new_ch)).unwrap();
                self.states.insert(listen_ch_id, State::TcpListener);
            }
            Some(State::TcpListener) => {
                let conn_ch_id = new_ch.handle().id();

                ch.send(Message::Open {
                    uri: Buffer::Static(b""),
                })
                .expect("failed to send accept message");

                ch.send(Message::Read {
                    offset: 0,
                    data: BufferMut::Vec(vec![0; RECV_BUFFER_SIZE]),
                })
                .expect("failed to send read message");

                let conn = Connection::new();
                eventloop.add_channel(Rc::new(new_ch)).unwrap();
                self.states.insert(conn_ch_id, State::TcpConn(conn));
            }
            _ => {
                trace!("unexpected open reply on {:?}", ch.handle().id());
            }
        }
    }

    fn on_read_reply(
        &mut self,
        eventloop: &mut EventLoop,
        ch: &Rc<Channel>,
        buf: BufferMut,
        len: usize,
    ) {
        let handle_id = ch.handle().id();
        let should_read_more = match self.states.get_mut(&handle_id) {
            Some(State::TcpConn(conn)) => {
                trace!("received {} bytes from {:?}", len, handle_id);

                let BufferMut::Vec(mut buf) = buf else {
                    unreachable!()
                };

                buf.truncate(len);
                matches!(conn.handle_recv(buf), ControlFlow::Continue(()))
            }
            _ => {
                trace!("unexpected read reply on {:?}", handle_id);
                return;
            }
        };

        if should_read_more {
            ch.send(Message::Read {
                offset: 0,
                data: BufferMut::Vec(vec![0; RECV_BUFFER_SIZE]),
            })
            .expect("failed to send read message");
        } else {
            self.send_next_or_close(eventloop, ch);
        }
    }

    fn on_error_reply(&mut self, eventloop: &mut EventLoop, ch: &Rc<Channel>, error: ErrorCode) {
        let handle_id = ch.handle().id();
        warn!("error reply from {:?}: {:?}", handle_id, error);

        if matches!(self.states.get(&handle_id), Some(State::TcpConn(_))) {
            self.close_channel(eventloop, handle_id);
        }
    }

    fn on_peer_closed(&mut self, eventloop: &mut EventLoop, ch: &Rc<Channel>) {
        let handle_id = ch.handle().id();
        trace!("peer closed: {:?}", handle_id);
        self.close_channel(eventloop, handle_id);
    }

    fn on_reply(&mut self, eventloop: &mut EventLoop, reply: ReplyEvent) {
        match reply {
            ReplyEvent::Open { ch, uri: _, new_ch } => {
                self.on_open_reply(eventloop, &ch, new_ch);
            }
            ReplyEvent::Read { ch, buf, len } => {
                self.on_read_reply(eventloop, &ch, buf, len);
            }
            ReplyEvent::Write { ch, buf: _, len: _ } => {
                self.send_next_or_close(eventloop, &ch);
            }
            ReplyEvent::Invoke {
                ch,
                input: _,
                output: _,
            } => {
                warn!("unexpected invoke reply from {:?}", ch.handle().id());
            }
            ReplyEvent::Error { ch, error } => {
                self.on_error_reply(eventloop, &ch, error);
            }
        }
    }
}

#[ftl::main]
fn main() {
    let mut eventloop = EventLoop::new().unwrap();
    let mut app = Main::new(&mut eventloop);

    loop {
        let event = eventloop.wait();
        match event {
            Event::Reply(reply) => app.on_reply(&mut eventloop, reply),
            Event::PeerClosed { ch } => app.on_peer_closed(&mut eventloop, &ch),
            event => {
                warn!("unhandled event: {:?}", event);
            }
        }
    }
}
