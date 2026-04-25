#![no_std]
#![no_main]

use ftl::channel::Channel;
use ftl::channel::Incoming;
use ftl::channel::Message;
use ftl::channel::MessageId;
use ftl::channel::OpenCompleter;
use ftl::channel::OpenOptions;
use ftl::channel::ReadRequest;
use ftl::channel::WriteCompleter;
use ftl::channel::WriteRequest;
use ftl::collections::HashMap;
use ftl::error::ErrorCode;
use ftl::handle::Handleable;
use ftl::prelude::*;
use ftl::sink::Event;
use ftl::sink::Sink;
use ftl::sync::Arc;
use ftl_tcpip::device::DeviceMap;
use ftl_tcpip::ethernet::MacAddr;
use ftl_tcpip::ip::IpAddr;
use ftl_tcpip::ip::ipv4::Ipv4Addr;
use ftl_tcpip::ip::ipv4::NetMask;
use ftl_tcpip::packet::Packet;
use ftl_tcpip::route::Route;
use ftl_tcpip::route::RouteTable;
use ftl_tcpip::socket::Endpoint;
use ftl_tcpip::socket::SocketMap;
use ftl_tcpip::transport::Port;
use ftl_tcpip::transport::tcp;
use ftl_tcpip::transport::tcp::TcpListener;

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
    mac_addr: MacAddr,
    driver_ch: Arc<Channel>,
}

impl ftl_tcpip::device::Device for MyDevice {
    fn mac_addr(&self) -> &MacAddr {
        &self.mac_addr
    }

    fn transmit(&mut self, pkt: &mut Packet) {
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

pub enum TcpWrite {
    Init(Option<WriteRequest<Arc<Channel>>>),
    Recved(WriteCompleter<Arc<Channel>>),
}

impl tcp::Write for TcpWrite {
    fn len(&self) -> usize {
        match self {
            Self::Init(Some(request)) => request.len(),
            Self::Init(None) => unreachable!(), // FIXME:
            Self::Recved(_completer) => unreachable!(), // FIXME:
        }
    }

    fn read(&mut self, buf: &mut [u8]) -> usize {
        match self {
            Self::Init(request) => {
                let request = request.take().unwrap();
                assert_eq!(buf.len(), request.len());
                match request.recv(buf) {
                    Ok((_, completer)) => {
                        *self = Self::Recved(completer);
                        buf.len()
                    }
                    Err(e) => {
                        warn!("failed to recv write body: {:?}", e.error());
                        0
                    }
                }
            }
            Self::Recved(_completer) => {
                warn!("failed to read tcp sock: already read");
                0
            }
        }
    }

    fn complete(self, result: Result<usize, tcp::Error>) {
        match (self, result) {
            (Self::Init(Some(request)), Ok(len)) => {
                request.reply(len);
            }
            (Self::Init(Some(request)), Err(e)) => {
                warn!("failed to write tcp sock: {:?}", e);
                request.reply_error(ErrorCode::InternalError) // FIXME:
            }
            (Self::Init(None), _) => unreachable!(), // FIXME:
            (Self::Recved(completer), Ok(len)) => {
                completer.reply(len);
            }
            (Self::Recved(completer), Err(e)) => {
                warn!("failed to write tcp sock: {:?}", e);
                completer.reply_error(ErrorCode::InternalError) // FIXME:
            }
        }
    }
}

pub struct TcpRead(ReadRequest<Arc<Channel>>);

impl tcp::Read for TcpRead {
    fn complete(self, result: Result<&[u8], tcp::Error>) {
        match result {
            Ok(buf) => self.0.reply(buf),
            Err(e) => {
                warn!("failed to read tcp sock: {:?}", e);
                self.0.reply_error(ErrorCode::InternalError) // FIXME:
            }
        }
    }
}

pub struct TcpAccept { 
    completer: OpenCompleter<Arc<Channel>>,
    their_ch: Channel,
}

impl tcp::Accept for TcpAccept {
    fn complete(self, result: Result<(), tcp::Error>) {
        match result {
            Ok(_) => self.completer.reply(self.their_ch.into_handle()),
            Err(e) => {
                warn!("failed to accept tcp sock: {:?}", e);
                self.completer.reply_error(ErrorCode::InternalError) // FIXME:
            }
        }
    }
}

#[derive(Debug)]
enum Context {
    Supervisor { ch: Channel },
    Driver { ch: Arc<Channel> },
    Client { ch: Channel },
    TcpListener { ch: Arc<Channel>, listener: Arc<TcpListener<TcpIpIo>> },
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

    let mut devices = DeviceMap::new();
    let device_id = devices
        .add(MyDevice {
            mac_addr: MacAddr::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]),
            driver_ch: driver_ch.clone(),
        })
        .unwrap();

    let mut routes = RouteTable::new();
    routes
        .add(Route::new(
            device_id,
            Ipv4Addr::new(10, 0, 2, 15),
            NetMask::new(255, 255, 255, 0),
        ))
        .unwrap();

    let mut sockets = SocketMap::new();

    let listener = sockets
        .tcp_listen::<TcpIpIo>(Endpoint {
            addr: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            port: Port::new(80),
        })
        .unwrap();

    let mut contexts = HashMap::new();
    contexts.insert(driver_ch.handle().id(), Context::Driver { ch: driver_ch });
    // contexts.insert(supervisor_ch.handle().id(), Context::Supervisor { ch: supervisor_ch });

    let mut pkt = Packet::new(RECV_BUFFER_SIZE, 0).unwrap();
    loop {
        let (id, event) = sink.wait().unwrap();
        let ctx = contexts.get(&id).unwrap();
        match (ctx, event) {
            (Context::Driver { ch }, Event::Message(peek)) => {
                match Incoming::parse(ch, peek) {
                    Incoming::ReadReply(reply) => {
                        let len = reply.read_len();
                        let buf = pkt.uninit_buf();
                        match reply.recv(&mut buf[..len]) {
                            Ok(_slice) => {
                                pkt.set_len(len);
                                ftl_tcpip::ethernet::handle_rx::<TcpIpIo>(
                                    &mut devices,
                                    &mut routes,
                                    &mut sockets,
                                    &mut pkt,
                                );

                                // Pull the next packet
                                ch
                                    .send(Message::Read {
                                        mid: MessageId::new(1),
                                        offset: 0,
                                        len: RECV_BUFFER_SIZE,
                                    })
                                    .unwrap();
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
            (Context::TcpListener { ch, listener }, Event::Message(peek)) => {
                match Incoming::parse(ch.clone(), peek) {
                    Incoming::Open(request) => {
                        let mut buf = vec![0; request.path_len()];
                        let completer = match request.recv(&mut buf) {
                            Ok((_, completer)) => completer,
                            Err(e) => {
                                warn!("failed to recv open request: {:?}", e.error());
                                e.reply_error(ErrorCode::InternalError);
                                continue;
                            }
                        };

                        let (their_ch, our_ch) = Channel::new().unwrap();
                        listener.accept(TcpAccept { completer, their_ch });
                        sink.add(&our_ch).unwrap();
                        contexts.insert(our_ch.handle().id(), Context::Client { ch: our_ch });
                    }
                    _ => {
                        warn!("unhandled message: {:?}", peek);
                    }
                }
            },
            (ctx, event) => {
                warn!("unhandled event for {:?}: {:?}", ctx, event);
            },
        }
    }
}
