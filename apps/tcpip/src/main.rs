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
use ftl_tcpip::TcpConnHandle;
use ftl_tcpip::TcpIp;
use ftl_tcpip::TcpListenerHandle;
use ftl_tcpip::ethernet::MacAddr;
use ftl_tcpip::ip::IpAddr;
use ftl_tcpip::ip::Ipv4Addr;
use ftl_tcpip::ip::NetMask;
use ftl_tcpip::packet::Packet;
use ftl_tcpip::route::Route;
use ftl_tcpip::socket::Endpoint;
use ftl_tcpip::transport::Port;

fn open_supervisor_channel(
    sink: &Sink,
    supervisor_ch: &Channel,
    path: &[u8],
    options: OpenOptions,
    description: &str,
) -> Channel {
    let mid = MessageId::new(1);
    supervisor_ch
        .send(Message::Open { mid, path, options })
        .unwrap();

    loop {
        let (id, event) = sink.wait().unwrap();
        match event {
            Event::Message(peek) if id == supervisor_ch.handle().id() => {
                match Incoming::parse(&supervisor_ch, peek) {
                    Incoming::OpenReply(reply) if reply.mid() == mid => {
                        match reply.recv() {
                            Ok(handle) => {
                                break Channel::from_handle(handle);
                            }
                            Err(error) => {
                                panic!("failed to open {description}: {:?}", error);
                            }
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

fn parse_tcp_listen_endpoint(path: &[u8]) -> Result<Endpoint, ErrorCode> {
    let path = core::str::from_utf8(path).map_err(|_| ErrorCode::InvalidArgument)?;
    let port_str = path
        .strip_prefix("tcp-listen:0.0.0.0:")
        .ok_or(ErrorCode::InvalidArgument)?;
    let port = port_str
        .parse::<u16>()
        .map_err(|_| ErrorCode::InvalidArgument)?;

    Ok(Endpoint {
        addr: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), // TODO:
        port: Port::new(port),
    })
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
    fn complete(self, tx_buffer: &mut ftl_tcpip::tcp::TcpBuffer) {
        tx_buffer.write_bytes_with(|buf| {
            let len = min(buf.len(), self.0.len());
            match self.0.recv(&mut buf[..len]) {
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
    fn complete(self, rx_buffer: &mut ftl_tcpip::tcp::TcpBuffer) {
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

#[derive(Debug, Clone)]
enum Context {
    Driver {
        ch: Arc<Channel>,
    },
    Server {
        ch: Arc<Channel>,
    },
    Client {
        ch: Arc<Channel>,
    },
    TcpConn {
        ch: Arc<Channel>,
        conn: TcpConnHandle<TcpIpIo>,
    },
    TcpListener {
        ch: Arc<Channel>,
        listener: TcpListenerHandle<TcpIpIo>,
    },
}

#[ftl::main]
fn main(supervisor_ch: Channel) {
    info!("Hello from tcpip");
    let sink = Sink::new().unwrap();
    sink.add(&supervisor_ch).unwrap();
    let server_ch = open_supervisor_channel(
        &sink,
        &supervisor_ch,
        b"service/tcpip",
        OpenOptions::LISTEN,
        "tcpip service",
    );
    info!("registered tcpip service: {:?}", server_ch.handle().id());

    let driver_ch = open_supervisor_channel(
        &sink,
        &supervisor_ch,
        b"service/ethernet",
        OpenOptions::CONNECT,
        "ethernet service",
    );
    sink.remove(supervisor_ch.handle().id()).unwrap();
    info!("connected to driver: {:?}", driver_ch.handle().id());

    let sink = Sink::new().unwrap();

    sink.add(&server_ch).unwrap();
    sink.add(&driver_ch).unwrap();
    let server_ch = Arc::new(server_ch);
    let driver_ch = Arc::new(driver_ch);

    driver_ch
        .send(Message::Read {
            mid: MessageId::new(1),
            offset: 0,
            len: RECV_BUFFER_SIZE,
        })
        .unwrap();

    let mut tcpip = TcpIp::<TcpIpIo>::new();
    let device_id = tcpip
        .add_device(MyDevice {
            mac_addr: MacAddr::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]),
            driver_ch: driver_ch.clone(),
        })
        .unwrap();

    tcpip
        .add_route(Route::new(
            device_id,
            Ipv4Addr::new(10, 0, 2, 15),
            NetMask::new(255, 255, 255, 0),
        ))
        .unwrap();

    let mut contexts = HashMap::new();
    contexts.insert(
        server_ch.handle().id(),
        Context::Server {
            ch: server_ch.clone(),
        },
    );
    contexts.insert(driver_ch.handle().id(), Context::Driver { ch: driver_ch });

    info!("tcpip server is ready");

    let mut pkt = Packet::new(RECV_BUFFER_SIZE, 0).unwrap();
    loop {
        let (id, event) = sink.wait().unwrap();
        let Some(ctx) = contexts.get(&id).cloned() else {
            warn!("event for unknown handle {:?}: {:?}", id, event);
            continue;
        };

        match (ctx, event) {
            (Context::Driver { ch }, Event::Message(peek)) => {
                match Incoming::parse(ch.clone(), peek) {
                    Incoming::ReadReply(reply) => {
                        let len = reply.read_len();
                        let buf = pkt.uninit_buf();
                        match reply.recv(&mut buf[..len]) {
                            Ok(_slice) => {
                                pkt.set_len(len);
                                tcpip.handle_rx(&mut pkt).unwrap();

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
                    Incoming::WriteReply(_reply) => {}
                    _ => {
                        warn!("unhandled message: {:?}", peek);
                    }
                }
            }
            (Context::Server { ch }, Event::Message(peek)) => {
                match Incoming::parse(ch.clone(), peek) {
                    Incoming::Open(request) => {
                        if request.options() != OpenOptions::CONNECT {
                            request.reply_error(ErrorCode::InvalidArgument);
                            continue;
                        }

                        let mut buf = vec![0; request.path_len()];
                        let completer = match request.recv(&mut buf) {
                            Ok((path, completer)) => {
                                if path != b"service/tcpip" {
                                    completer.reply_error(ErrorCode::InvalidArgument);
                                    continue;
                                }
                                completer
                            }
                            Err(e) => {
                                warn!("failed to recv service open request: {:?}", e.error());
                                e.reply_error(ErrorCode::Overloaded);
                                continue;
                            }
                        };

                        let (our_ch, their_ch) = match Channel::new() {
                            Ok(pair) => pair,
                            Err(error) => {
                                completer.reply_error(error);
                                continue;
                            }
                        };

                        if let Err(error) = sink.add(&our_ch) {
                            warn!("failed to add tcpip client channel to sink: {:?}", error);
                            completer.reply_error(ErrorCode::OutOfResources);
                            continue;
                        }

                        let our_ch = Arc::new(our_ch);
                        contexts.insert(our_ch.handle().id(), Context::Client { ch: our_ch });
                        completer.reply(their_ch.into_handle());
                    }
                    _ => {
                        warn!("unhandled service message: {:?}", peek);
                    }
                }
            }
            (Context::Client { ch }, Event::Message(peek)) => {
                match Incoming::parse(ch.clone(), peek) {
                    Incoming::Open(request) => {
                        if request.options() != OpenOptions::LISTEN {
                            request.reply_error(ErrorCode::InvalidArgument);
                            continue;
                        }

                        let mut buf = vec![0; request.path_len()];
                        let completer = match request.recv(&mut buf) {
                            Ok((path, completer)) => {
                                let endpoint = match parse_tcp_listen_endpoint(path) {
                                    Ok(endpoint) => endpoint,
                                    Err(error) => {
                                        completer.reply_error(error);
                                        continue;
                                    }
                                };
                                (endpoint, completer)
                            }
                            Err(e) => {
                                warn!("failed to recv listen request: {:?}", e.error());
                                e.reply_error(ErrorCode::Overloaded);
                                continue;
                            }
                        };

                        let (endpoint, completer) = completer;
                        let (our_ch, their_ch) = match Channel::new() {
                            Ok(pair) => pair,
                            Err(error) => {
                                completer.reply_error(error);
                                continue;
                            }
                        };

                        if let Err(error) = sink.add(&our_ch) {
                            warn!("failed to add listener channel to sink: {:?}", error);
                            completer.reply_error(ErrorCode::OutOfResources);
                            continue;
                        }

                        let listener = match tcpip.tcp_listen(endpoint) {
                            Ok(listener) => listener,
                            Err(_error) => {
                                let _ = sink.remove(our_ch.handle().id());
                                completer.reply_error(ErrorCode::OutOfResources);
                                continue;
                            }
                        };

                        let our_ch = Arc::new(our_ch);
                        info!("listening on TCP {}:{}", endpoint.addr, endpoint.port);
                        contexts.insert(
                            our_ch.handle().id(),
                            Context::TcpListener {
                                ch: our_ch,
                                listener,
                            },
                        );
                        completer.reply(their_ch.into_handle());
                    }
                    _ => {
                        warn!("unhandled tcpip client message: {:?}", peek);
                    }
                }
            }
            (Context::TcpListener { ch, listener }, Event::Message(peek)) => {
                match Incoming::parse(ch.clone(), peek) {
                    Incoming::Open(request) => {
                        if request.options() != OpenOptions::CONNECT {
                            request.reply_error(ErrorCode::InvalidArgument);
                            continue;
                        }

                        let mut buf = vec![0; request.path_len()];
                        let completer = match request.recv(&mut buf) {
                            Ok((_, completer)) => completer,
                            Err(e) => {
                                warn!("failed to recv open request: {:?}", e.error());
                                e.reply_error(ErrorCode::Overloaded);
                                continue;
                            }
                        };

                        let (our_ch, their_ch) = match Channel::new() {
                            Ok(pair) => pair,
                            Err(error) => {
                                completer.reply_error(error);
                                continue;
                            }
                        };
                        if let Err(error) = sink.add(&our_ch) {
                            warn!("failed to add accepted connection to sink: {:?}", error);
                            completer.reply_error(ErrorCode::OutOfResources);
                            continue;
                        }

                        let conn = tcpip.tcp_accept(
                            listener,
                            TcpAccept {
                                completer,
                                their_ch,
                            },
                        );

                        let our_ch = Arc::new(our_ch);
                        contexts
                            .insert(our_ch.handle().id(), Context::TcpConn { ch: our_ch, conn });
                    }
                    _ => {
                        warn!("unhandled message: {:?}", peek);
                    }
                }
            }
            (Context::TcpConn { ch, conn }, Event::Message(peek)) => {
                match Incoming::parse(ch.clone(), peek) {
                    Incoming::Read(request) => {
                        tcpip.tcp_read(conn, TcpRead(request));
                    }
                    Incoming::Write(request) => {
                        tcpip.tcp_write(conn, TcpWrite(request));
                    }
                    _ => {
                        warn!("unhandled tcp connection message: {:?}", peek);
                    }
                }
            }
            (Context::TcpConn { conn, .. }, Event::PeerClosed) => {
                trace!("tcp connection peer closed: {:?}", id);
                tcpip.tcp_close(conn);
                if let Err(error) = sink.remove(id) {
                    warn!("failed to remove handle from sink: {:?}", error);
                }
                contexts.remove(&id);
            }
            (_, Event::PeerClosed) => {
                trace!("peer closed: {:?}", id);
                if let Err(error) = sink.remove(id) {
                    warn!("failed to remove handle from sink: {:?}", error);
                }
                contexts.remove(&id);
            }
            (ctx, event) => {
                warn!("unhandled event for {:?}: {:?}", ctx, event);
            }
        }
    }
}
