use ftl_api::channel::Channel;
use ftl_api::channel::ChannelSender;
use ftl_api::collections::HashMap;
use ftl_api::mainloop::Mainloop;
use ftl_api::prelude::*;
use ftl_api::types::error::FtlError;
use smoltcp::iface::Config;
use smoltcp::iface::Interface;
use smoltcp::iface::SocketHandle;
use smoltcp::iface::SocketSet;
use smoltcp::socket::tcp;
use smoltcp::time::Instant;
use smoltcp::wire::HardwareAddress;
use smoltcp::wire::IpAddress;
use smoltcp::wire::IpCidr;
use smoltcp::wire::IpListenEndpoint;

use crate::device::NetDevice;

fn now() -> Instant {
    // FIXME:
    Instant::from_millis(0)
}

enum SocketState {
    Listening {
        listen_sender: ChannelSender,
        listen_endpoint: IpListenEndpoint,
    },
    Established {
        sender: ChannelSender,
    },
}

struct Socket {
    smol_handle: SocketHandle,
    state: SocketState,
}

pub struct TcpIp<'a> {
    smol_sockets: SocketSet<'a>,
    device: NetDevice,
    iface: Interface,
    sockets: HashMap<SocketHandle, Socket>,
}

impl<'a> TcpIp<'a> {
    pub fn new(mut device: NetDevice, hwaddr: HardwareAddress) -> TcpIp<'a> {
        let config = Config::new(hwaddr.into());
        let mut iface = Interface::new(config, &mut device, now());
        let smol_sockets = SocketSet::new(Vec::with_capacity(16));

        // FIXME:
        iface.update_ip_addrs(|ip_addrs| {
            ip_addrs
                .push(IpCidr::new(IpAddress::v4(10, 0, 2, 15), 24))
                .unwrap();
        });

        TcpIp {
            device,
            iface,
            smol_sockets,
            sockets: HashMap::new(),
        }
    }

    pub fn poll(&mut self, mainloop: &mut Mainloop<Context, Message>) {
        while self
            .iface
            .poll(now(), &mut self.device, &mut self.smol_sockets)
        {
            let mut closed_sockets = Vec::new();
            let mut new_sockets = Vec::new();
            for (handle, sock) in self.sockets.iter_mut() {
                let smol_sock = self.smol_sockets.get_mut::<tcp::Socket>(sock.smol_handle);
                let mut close = false;
                match (&mut sock.state, smol_sock.state()) {
                    (
                        SocketState::Listening { .. },
                        tcp::State::Listen | tcp::State::SynReceived,
                    ) => {}
                    (
                        SocketState::Listening {
                            listen_sender,
                            listen_endpoint,
                        },
                        tcp::State::Established,
                    ) => {
                        let (their_ch, our_ch) = Channel::create().unwrap();
                        listen_sender
                            .send(TcpAccepted {
                                conn: their_ch.into(),
                            })
                            .unwrap();

                        let (our_ch_sender, our_ch_receiver) = our_ch.split();
                        mainloop
                            .add_channel(
                                (our_ch_sender.clone(), our_ch_receiver),
                                Context::DataSocket(sock.smol_handle),
                            )
                            .unwrap();

                        let new_listen_sock_handle = self.smol_sockets.add(tcp::Socket::new(
                            tcp::SocketBuffer::new(vec![0; 8192]),
                            tcp::SocketBuffer::new(vec![0; 8192]),
                        ));
                        let new_listen_sock = self
                            .smol_sockets
                            .get_mut::<tcp::Socket>(new_listen_sock_handle);
                        new_listen_sock.listen(*listen_endpoint).unwrap();
                        new_sockets.push(Socket {
                            smol_handle: new_listen_sock_handle,
                            state: SocketState::Listening {
                                listen_endpoint: *listen_endpoint,
                                listen_sender: listen_sender.clone(),
                            },
                        });

                        sock.state = SocketState::Established {
                            sender: our_ch_sender,
                        };
                    }
                    (SocketState::Listening { .. }, _) => {
                        // Inactive, closed, or unknown state. Close the socket.
                        close = true;
                    }
                    (SocketState::Established { sender: ch }, _) if smol_sock.can_recv() => {
                        loop {
                            let mut buf = [0; 2048];
                            let len = smol_sock.recv_slice(&mut buf).unwrap();
                            if len == 0 {
                                break;
                            }

                            // FIXME: Backpressure
                            ch.send(TcpReceived {
                                data: buf[..len].try_into().unwrap(),
                            })
                            .unwrap();
                        }
                    }
                    (SocketState::Established { .. }, tcp::State::Established) => {
                        // Do nothing.
                    }
                    (SocketState::Established { .. }, _) => {
                        close = true;
                    }
                }

                if close {
                    debug_warn!("closing socket");
                    closed_sockets.push(*handle);
                }
            }

            for handle in closed_sockets {
                let sock = self.sockets.remove(&handle).unwrap();
                self.smol_sockets.remove(sock.smol_handle);

                match sock.state {
                    SocketState::Established { sender } => {
                        sender.send(TcpClosed {}).unwrap();
                        mainloop.remove(sender.handle().id()).unwrap();
                    }
                    _ => {
                        unreachable!();
                    }
                }
            }

            for socket in new_sockets {
                self.sockets.insert(socket.smol_handle, socket);
            }
        }
    }

    pub fn tcp_listen(&mut self, port: u16) -> Result<Channel, FtlError> {
        let (our_listen_ch, their_listen_ch) = Channel::create()?;
        let listen_endpoint = IpListenEndpoint { addr: None, port };

        let rx_buf = tcp::SocketBuffer::new(vec![0; 8192]);
        let tx_buf = tcp::SocketBuffer::new(vec![0; 8192]);
        let mut sock = tcp::Socket::new(rx_buf, tx_buf);
        sock.listen(listen_endpoint).unwrap();

        let (listen_sender, _) = our_listen_ch.split();

        info!("listening on port {}", port);
        let handle = self.smol_sockets.add(sock);
        self.sockets.insert(
            handle,
            Socket {
                smol_handle: handle,
                state: SocketState::Listening {
                    listen_sender,
                    listen_endpoint,
                },
            },
        );

        Ok(their_listen_ch)
    }

    pub fn tcp_send(&mut self, handle: SocketHandle, data: &[u8]) -> Result<(), FtlError> {
        let socket = self
            .sockets
            .get_mut(&handle)
            .ok_or(FtlError::HandleNotFound)?;
        if !matches!(socket.state, SocketState::Established { .. }) {
            return Err(FtlError::InvalidState);
        }

        self.smol_sockets
            .get_mut::<tcp::Socket>(socket.smol_handle)
            .send_slice(data)
            .unwrap();
        Ok(())
    }

    pub fn receive_pkt(&mut self, pkt: &[u8]) {
        self.device.receive_pkt(pkt);
    }
}
