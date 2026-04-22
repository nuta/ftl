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
use ftl_tcpip::ethernet::MacAddr;
use ftl_tcpip::ip::ipv4::Ipv4Addr;
use ftl_tcpip::ip::ipv4::NetMask;
use ftl_tcpip::packet::Packet;
use ftl_tcpip::route::Route;
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
    type Device = MyDevice;
    type TcpWrite = TcpWrite;
    type TcpRead = TcpRead;
    type TcpAccept = TcpAccept;
}

pub struct MyDevice {
    driver_ch: Arc<Channel>,
}

impl ftl_tcpip::Device for MyDevice {
    fn transmit(&self, pkt: &mut Packet) {
        info!("transmitting packet: {:?}", pkt.len());
        let m = Message::Write {
            mid: MessageId::new(1),
            offset: 0,
            buf: pkt.slice(),
        };

        if let Err(e) = self.driver_ch.send(m) {
            warn!("failed to send message: {:?}", e);
        }
    }
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

    let driver_ch = Arc::new(driver_ch);
    driver_ch
        .send(Message::Read {
            mid: MessageId::new(1),
            offset: 0,
            len: RECV_BUFFER_SIZE,
        })
        .unwrap();

    let mut routes = RouteTable::new();
    routes.add(Route::new(MyDevice { driver_ch: driver_ch.clone() }, Ipv4Addr::new(10, 0, 0, 1), NetMask::new(255, 255, 255, 0), MacAddr::new([0x02, 0x00, 0x00, 0x00, 0x00, 0x00]))).unwrap();

    let mut pkt = Packet::new(RECV_BUFFER_SIZE).unwrap();
    let mut sockets = SocketMap::new();
    loop {
        let (id, event) = sink.wait().unwrap();
        match event {
            Event::Message(peek) if id == driver_ch.handle().id() => {
                match Incoming::parse(&driver_ch, peek) {
                    Incoming::ReadReply(reply) => {
                        let len = reply.read_len();
                        let buf = pkt.uninit_buf();
                        match reply.recv(&mut buf[..len]) {
                            Ok(slice) => {
                                pkt.set_len(len);
                                ftl_tcpip::receive_packet::<TcpIpIo>(
                                    &mut sockets,
                                    &mut routes,
                                    &mut pkt,
                                );
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
