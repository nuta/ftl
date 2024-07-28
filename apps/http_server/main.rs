#![no_std]
#![no_main]
#![feature(slice_split_once)]

use ftl_api::channel::Channel;
use ftl_api::handle::OwnedHandle;
use ftl_api::mainloop::Event;
use ftl_api::mainloop::Mainloop;
use ftl_api::prelude::*;
use ftl_api::types::message::MessageBuffer;
use ftl_api_autogen::apps::http_server::Environ;
use ftl_api_autogen::apps::http_server::Message;

#[derive(Debug)]
enum State {
    Headers,
    Body,
    Eof,
}

#[derive(Debug)]
struct Client {
    state: State,
    buffered: Vec<u8>,
}

impl Client {
    pub fn new() -> Client {
        Client {
            state: State::Headers,
            buffered: Vec::new(),
        }
    }

    pub fn receive(&mut self, data: &[u8]) {
        match self.state {
            State::Headers => {
                self.buffered.extend_from_slice(&data);
                if let Some(index) = self.buffered.windows(4).position(|w| w == b"\r\n\r\n") {
                    let request_bytes = &self.buffered[..index + 4];

                    let mut headers = [httparse::EMPTY_HEADER; 32];
                    let mut request = httparse::Request::new(&mut headers);
                    match request.parse(request_bytes) {
                        Ok(httparse::Status::Complete(len)) => {
                            trace!("parsed request: {:?}", request);
                            self.state = State::Body;
                            self.buffered = Vec::new();
                        }
                        Ok(httparse::Status::Partial) => {
                            warn!("partial request");
                            return;
                        }
                        Err(e) => {
                            warn!("error parsing request: {:?}", e);
                            return;
                        }
                    };
                }
            }
            State::Body => {
            }
            State::Eof => {
                warn!("received data after EOF");
            }
        }
    }
}

#[derive(Debug)]
enum Context {
    Autopilot,
    Tcpip,
    TcpSock(Client),
}

#[ftl_api::main]
pub fn main(mut env: Environ) {
    info!("starting");
    let tcpip_ch = env.depends.tcpip.take().unwrap();
    let mut buffer = MessageBuffer::new();
    let mut mainloop = Mainloop::<Context, Message>::new().unwrap();
    mainloop
        .add_channel(env.autopilot_ch.take().unwrap(), Context::Autopilot)
        .unwrap();
    mainloop.add_channel(tcpip_ch, Context::Tcpip).unwrap();

    loop {
        trace!("waiting for event...");
        match mainloop.next(&mut buffer) {
            Event::Message { ctx, ch, m } => {
                match (ctx, m) {
                    (Context::Tcpip, Message::TcpAccepted(m)) => {
                        let ch = Channel::from_handle(OwnedHandle::from_raw(m.sock()));
                        mainloop.add_channel(ch, Context::TcpSock(Client::new())).unwrap();
                    }
                    (Context::TcpSock(client), Message::TcpReceived(m)) => {
                        client.receive(m.data().as_slice());
                    }
                    (_, m) => {
                        warn!("unexpected message: {:?}", m);
                    }
                }
            }
            ev => {
                warn!("unexpected event: {:?}", ev);
            }
        }
    }
}
