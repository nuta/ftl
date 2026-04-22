#![no_std]
#![no_main]

use ftl::channel::Channel;
use ftl::channel::Incoming;
use ftl::channel::Message;
use ftl::channel::MessageId;
use ftl::channel::OpenOptions;
use ftl::handle::Handleable;
use ftl::prelude::*;
use ftl::sink::Event;
use ftl::sink::Sink;
use ftl::sync::Arc;
use ftl_tcpip::route::RouteTable;
use ftl_tcpip::socket::SocketMap;
use ftl_tcpip::transport::tcp;

fn conenct_to_driver(supervisor_ch: &Channel) -> Channel {
    let sink = Sink::new().unwrap();
    sink.add(supervisor_ch).unwrap();

    // Ask the supervisor process to connect to the driver.
    let mid = MessageId::new(1);
    let path = b"service/ethernet";
    let options = OpenOptions::CONNECT;
    supervisor_ch
        .send(Message::Open {
            mid,
            path: path.as_slice(),
            options,
        })
        .unwrap();

    let driver_ch = loop {
        let (id, event) = sink.wait().unwrap();
        match event {
            Event::Message(peek) if id == supervisor_ch.handle().id() => {
                match Incoming::parse(&supervisor_ch, peek) {
                    Incoming::OpenReply(reply) => {
                        match reply.recv() {
                            Ok(handle) => {
                                break Channel::from_handle(handle);
                            }
                            Err(error) => {
                                panic!("failed to recv with handle: {:?}", error);
                            }
                        }
                    }
                    _ => {
                        warn!("unhandled message: {:?}", peek);
                    }
                }
            }
            _ => {
                warn!("unhandled event: {:?}", event);
            }
        }
    };

    driver_ch
}

const RECV_BUFFER_SIZE: usize = 1514;

pub struct TcpIpIo;

impl ftl_tcpip::Io for TcpIpIo {
    type TcpWrite = TcpWrite;
    type TcpRead = TcpRead;
    type TcpAccept = TcpAccept;
}

pub struct TcpWrite {}

impl tcp::Write for TcpWrite {
    fn read(&mut self, buf: &mut [u8]) -> usize {
        todo!()
    }

    fn complete(self, result: Result<usize, tcp::Error>) {
        todo!()
    }
}

pub struct TcpRead {}

impl tcp::Read for TcpRead {
    fn write(&mut self, buf: &[u8]) -> usize {
        todo!()
    }

    fn complete(self, result: Result<usize, tcp::Error>) {
        todo!()
    }
}

pub struct TcpAccept {}

impl tcp::Accept for TcpAccept {
    fn complete(self, result: Result<(), tcp::Error>) {
        todo!()
    }
}

#[ftl::main]
fn main(supervisor_ch: Channel) {
    info!("Hello from tcpip");
    let driver_ch = conenct_to_driver(&supervisor_ch);
    info!("connected to driver: {:?}", driver_ch.handle().id());

    let sink = Sink::new().unwrap();
    sink.add(&driver_ch).unwrap();

    driver_ch
        .send(Message::Read {
            mid: MessageId::new(1),
            offset: 0,
            len: RECV_BUFFER_SIZE,
        })
        .unwrap();

    let mut buf = [0; RECV_BUFFER_SIZE];
    let mut sockets = SocketMap::new();
    let mut routes = RouteTable::new();
    loop {
        let (id, event) = sink.wait().unwrap();
        match event {
            Event::Message(peek) if id == driver_ch.handle().id() => {
                match Incoming::parse(&driver_ch, peek) {
                    Incoming::ReadReply(reply) => {
                        let len = reply.read_len();
                        match reply.recv(&mut buf[..len]) {
                            Ok(slice) => {
                                ftl_tcpip::receive_packet::<TcpIpIo>(&sockets, &routes, &slice);
                            }
                            Err(error) => {
                                panic!("failed to recv with error: {:?}", error);
                            }
                        }
                    }
                    _ => {
                        warn!("unhandled message: {:?}", peek);
                    }
                }
            }
            _ => {
                warn!("unhandled event: {:?}", event);
            }
        }
    }
}
