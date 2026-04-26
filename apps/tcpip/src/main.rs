#![no_std]
#![no_main]

use core::cmp::min;

use ftl::channel::Channel;
use ftl::channel::Incoming;
use ftl::channel::Message;
use ftl::channel::MessageId;
use ftl::channel::OpenCompleter;
use ftl::channel::OpenOptions;
use ftl::channel::ReadRequest;
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
use ftl_tcpip::tcp::TcpConn;
use ftl_tcpip::tcp::TcpListener;
use ftl_tcpip::transport::Port;

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

pub struct TcpWrite(WriteRequest<Arc<Channel>>);

impl ftl_tcpip::tcp::Write for TcpWrite {
    fn complete(self, tx_buffer: &mut ftl_tcpip::tcp::RingBuffer) {
        tx_buffer.write_bytes_with(|buf| {
            let len = min(buf.len(), self.0.len());
            match self.0.recv(buf) {
                Ok((_, completer)) => {
                    completer.reply(len);
                    len
                }
                Err(e) => {
                    warn!("failed to recv with error: {:?}", e.error());
                    0
                }
            }
        });
    }
}

pub struct TcpRead(ReadRequest<Arc<Channel>>);

impl ftl_tcpip::tcp::Read for TcpRead {
    fn complete(self, rx_buffer: &mut ftl_tcpip::tcp::RingBuffer) {
        rx_buffer.read_bytes_with(self.0.len(), |buf| {
            let Some(buf) = buf else {
                // This should not happen.
                return 0;
            };

            // FIXME: Consider max body length in IPC?

            self.0.reply(buf);
            buf.len()
        });
    }
}

pub struct TcpAccept {
    completer: OpenCompleter<Arc<Channel>>,
    their_ch: Channel,
}

impl ftl_tcpip::tcp::Accept for TcpAccept {
    fn complete(self, result: Result<(), ftl_tcpip::tcp::Error>) {
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
    Supervisor {
        ch: Channel,
    },
    Driver {
        ch: Arc<Channel>,
    },
    Client {
        ch: Channel,
        conn: Arc<TcpConn<TcpIpIo>>,
    },
    TcpListener {
        ch: Arc<Channel>,
        listener: Arc<TcpListener<TcpIpIo>>,
    },
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
                                ch.send(Message::Read {
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
                        let conn = match listener.accept(
                            &mut sockets,
                            TcpAccept {
                                completer,
                                their_ch,
                            },
                        ) {
                            Ok(conn) => conn,
                            Err(e) => {
                                warn!("failed to accept tcp sock: {:?}", e);
                                continue;
                            }
                        };

                        sink.add(&our_ch).unwrap();
                        contexts.insert(our_ch.handle().id(), Context::Client { ch: our_ch, conn });
                    }
                    _ => {
                        warn!("unhandled message: {:?}", peek);
                    }
                }
            }
            (ctx, event) => {
                warn!("unhandled event for {:?}: {:?}", ctx, event);
            }
        }
    }
}
