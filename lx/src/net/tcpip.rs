use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem::offset_of;
use core::num::NonZeroU16;
use core::num::NonZeroU32;

use ftl::net::Net;
use ftl::poll::Poll;
use ftl::time::MonoTime;
use ftl::trace;
use ftl_types::error::ErrorCode;
use ftl_types::net::ETHTYPE_IPV4;
use ftl_types::net::IPPROTO_TCP;
use ftl_types::net::IPPROTO_UDP;
use ftl_types::net::Rule;
use ftl_utils::spinlock::SpinLock;
use hashbrown::HashMap;

use super::ipv4::Endpoint;
use super::ipv4::Ipv4Header;
use super::tcp::HeaderBuilder;
use super::tcp::Segment;
use super::tcp::TcpConn;
use super::tcp::TcpFlows;
use super::tcp::TcpListener;
use super::tcp::TcpPacketInfo;
use super::tcp::TcpSocket;
use super::udp::HeaderBuilder as UdpHeaderBuilder;
use super::udp::RxQueue;
use super::udp::UdpPacketInfo;
use super::udp::UdpSocket;
use crate::types::errno::Errno;
use crate::wait_queue::WaitQueue;

const RX_HEADER_LEN: usize = 128;
const RX_PAYLOAD_LEN: usize = 2048;
const EPHEMERAL_PORT_MIN: u16 = 49152;
const EPHEMERAL_PORT_MAX: u16 = 65535;

fn alloc_buf(len: usize) -> Result<Vec<u8>, ErrorCode> {
    let mut buf = Vec::new();
    buf.try_reserve_exact(len)
        .map_err(|_| ErrorCode::OutOfMemory)?;
    buf.resize(len, 0);
    Ok(buf)
}

pub struct TcpIp {
    net: Net,
    tcp_flows: SpinLock<TcpFlows>,
    tcp_listeners: SpinLock<HashMap<u16, Arc<TcpListener>>>,
    udp_ports: SpinLock<HashMap<u16, Arc<RxQueue>>>,
}

impl TcpIp {
    pub fn new(net: Net) -> Arc<Self> {
        Arc::new(Self {
            net,
            tcp_flows: SpinLock::new(TcpFlows::new()),
            tcp_listeners: SpinLock::new(HashMap::new()),
            udp_ports: SpinLock::new(HashMap::new()),
        })
    }

    pub fn subscribe(&self, poll: &Poll, cookie: usize) -> Result<(), ErrorCode> {
        self.net.subscribe(poll, cookie)
    }

    pub fn create_tcp_socket(self: &Arc<Self>) -> Result<Arc<TcpSocket>, ErrorCode> {
        TcpSocket::new(self.clone())
    }

    pub fn create_udp_socket(self: &Arc<Self>) -> Result<Arc<UdpSocket>, ErrorCode> {
        UdpSocket::new(self.clone())
    }

    pub fn send_tcp_segment(
        &self,
        remote: Endpoint,
        local_port: u16,
        segment: Segment,
    ) -> Result<(), ErrorCode> {
        let header = HeaderBuilder::new().build(remote, local_port, &segment);
        self.net.send(&header, segment.payload)
    }

    pub fn bind_tcp_listener(
        &self,
        port: u16,
        listener: Arc<TcpListener>,
    ) -> Result<(), ErrorCode> {
        let local_port = NonZeroU16::new(port).ok_or(ErrorCode::InvalidArg)?;
        let rule = Rule::new(
            ETHTYPE_IPV4,
            IPPROTO_TCP,
            None,
            Some(local_port),
            None,
            None,
        );
        self.net.bind(&rule)?;
        self.tcp_listeners.lock().insert(port, listener);
        Ok(())
    }

    fn bind_tcp_conn(
        &self,
        local_port: Option<u16>,
        remote: Endpoint,
    ) -> Result<(u16, Rule), Errno> {
        let remote_ip = NonZeroU32::new(remote.ip).ok_or(Errno::EINVAL)?;
        let remote_port = NonZeroU16::new(remote.port).ok_or(Errno::EINVAL)?;

        let local_port = match local_port {
            Some(0) | None => None,
            Some(port) => NonZeroU16::new(port),
        };

        // Bind to a specific port.
        if let Some(local_port) = local_port {
            let rule = Rule::new(
                ETHTYPE_IPV4,
                IPPROTO_TCP,
                None,
                Some(local_port),
                Some(remote_ip),
                Some(remote_port),
            );

            return match self.net.bind(&rule) {
                Ok(()) => Ok((local_port.get(), rule)),
                Err(ErrorCode::AlreadyExists) => Err(Errno::EADDRINUSE),
                Err(error) => Err(error.into()),
            };
        }

        // Start scanning from a random port.
        let mut seed = [0; 2];
        crate::random::read(&mut seed)?;
        let num_ports = EPHEMERAL_PORT_MAX - EPHEMERAL_PORT_MIN + 1;
        let start = u16::from_ne_bytes(seed) % num_ports;

        // TODO: Add a system call to assign an ephemeral port.
        for i in 0..num_ports {
            let port = EPHEMERAL_PORT_MIN + (start + i) % num_ports;
            let rule = Rule::new(
                ETHTYPE_IPV4,
                IPPROTO_TCP,
                None, // Kernel chooses the local IP
                NonZeroU16::new(port),
                Some(remote_ip),
                Some(remote_port),
            );

            match self.net.bind(&rule) {
                Ok(()) => return Ok((port, rule)),
                Err(ErrorCode::AlreadyExists) => {
                    // The port is already in use. Try the next one.
                }
                Err(error) => return Err(error.into()),
            }
        }

        Err(Errno::EADDRNOTAVAIL)
    }

    pub fn bind_udp(&self, local_port: Option<u16>, rx_queue: Arc<RxQueue>) -> Result<u16, Errno> {
        let local_port = match local_port {
            Some(0) | None => None,
            Some(port) => NonZeroU16::new(port),
        };

        if let Some(local_port) = local_port {
            let rule = Rule::new(
                ETHTYPE_IPV4,
                IPPROTO_UDP,
                None,
                Some(local_port),
                None,
                None,
            );

            match self.net.bind(&rule) {
                Ok(()) => {
                    self.udp_ports.lock().insert(local_port.get(), rx_queue);
                    return Ok(local_port.get());
                }
                Err(ErrorCode::AlreadyExists) => return Err(Errno::EADDRINUSE),
                Err(error) => return Err(error.into()),
            }
        }

        // Assign an ephemeral port.
        for port in EPHEMERAL_PORT_MIN..=EPHEMERAL_PORT_MAX {
            let rule = Rule::new(
                ETHTYPE_IPV4,
                IPPROTO_UDP,
                None,
                NonZeroU16::new(port),
                None,
                None,
            );

            match self.net.bind(&rule) {
                Ok(()) => {
                    self.udp_ports.lock().insert(port, rx_queue);
                    return Ok(port);
                }
                Err(ErrorCode::AlreadyExists) => {
                    // The port is already in use. Try the next one.
                }
                Err(error) => return Err(error.into()),
            }
        }

        Err(Errno::EADDRINUSE)
    }

    pub fn unbind_udp(&self, port: u16) {
        self.udp_ports.lock().remove(&port);
        let rule = Rule::new(
            ETHTYPE_IPV4,
            IPPROTO_UDP,
            None,
            NonZeroU16::new(port),
            None,
            None,
        );

        let _ = self.net.unbind(&rule);
    }

    pub fn send_udp(
        &self,
        remote: Endpoint,
        local_port: u16,
        payload: &[u8],
    ) -> Result<(), ErrorCode> {
        let builder = UdpHeaderBuilder::new();
        let header = builder.build(remote, local_port, payload.len());
        self.net.send(&header, payload)
    }

    pub fn connect_tcp(
        self: &Arc<Self>,
        remote: Endpoint,
        local_port: Option<u16>,
        wait_queue: Arc<WaitQueue>,
    ) -> Result<Arc<TcpConn>, Errno> {
        let mut iss = [0; 4];
        crate::random::read(&mut iss)?;

        let (local_port, rule) = self.bind_tcp_conn(local_port, remote)?;
        let conn = match TcpConn::new_active(
            self.clone(),
            wait_queue,
            remote,
            local_port,
            u32::from_ne_bytes(iss),
        ) {
            Ok(conn) => conn,
            Err(error) => {
                let _ = self.net.unbind(&rule);
                return Err(error.into());
            }
        };

        // Register the flow.
        self.tcp_flows
            .lock()
            .insert(local_port, remote, conn.clone(), rule);

        // Start handshake.
        conn.start_active();
        Ok(conn)
    }

    pub fn add_tcp_conn(&self, conn: Arc<TcpConn>, pkt: &TcpPacketInfo) -> Result<(), ErrorCode> {
        let local_port = NonZeroU16::new(pkt.local_port).ok_or(ErrorCode::InvalidArg)?;
        let remote_ip = NonZeroU32::new(pkt.remote_ip).ok_or(ErrorCode::InvalidArg)?;
        let remote_port = NonZeroU16::new(pkt.remote_port).ok_or(ErrorCode::InvalidArg)?;
        let rule = Rule::new(
            ETHTYPE_IPV4,
            IPPROTO_TCP,
            None,
            Some(local_port),
            Some(remote_ip),
            Some(remote_port),
        );

        self.net.bind(&rule)?;
        let remote = Endpoint {
            ip: pkt.remote_ip,
            port: pkt.remote_port,
        };
        self.tcp_flows
            .lock()
            .insert(pkt.local_port, remote, conn, rule);
        Ok(())
    }

    pub fn unbind_tcp_listener(&self, port: u16) {
        self.tcp_listeners.lock().remove(&port);
        let Some(local_port) = NonZeroU16::new(port) else {
            return;
        };
        let rule = Rule::new(
            ETHTYPE_IPV4,
            IPPROTO_TCP,
            None,
            Some(local_port),
            None,
            None,
        );
        let _ = self.net.unbind(&rule);
    }

    pub fn handle_rx(&self) {
        let Ok(mut header) = alloc_buf(RX_HEADER_LEN) else {
            trace!("failed to allocate RX header buffer");
            return;
        };
        let Ok(mut payload) = alloc_buf(RX_PAYLOAD_LEN) else {
            trace!("failed to allocate RX payload buffer");
            return;
        };

        loop {
            let payload_len = match self.net.recv(&mut header, &mut payload) {
                Ok(len) => len,
                Err(error) if error == ErrorCode::Empty => return,
                Err(_) => panic!("failed to receive a network packet"),
            };

            let payload = &payload[..payload_len];
            match header[offset_of!(Ipv4Header, protocol)] {
                IPPROTO_TCP => self.handle_tcp_rx(&header, payload),
                IPPROTO_UDP => self.handle_udp_rx(&header, payload),
                _ => {}
            }
        }
    }

    fn handle_tcp_rx(&self, header: &[u8], payload: &[u8]) {
        let pkt = TcpPacketInfo::parse(header);

        // Lookup the flow or listener.
        let conn = self.tcp_flows.lock().lookup(&pkt);
        if let Some(conn) = conn {
            conn.handle_rx(&pkt, payload);
        } else {
            let listener = self.tcp_listeners.lock().get(&pkt.local_port).cloned();
            if let Some(listener) = listener {
                listener.handle_rx(&pkt, payload);
            }
        }

        self.garbage_collect();
    }

    fn handle_udp_rx(&self, header: &[u8], payload: &[u8]) {
        let pkt = UdpPacketInfo::parse(header);

        let Some(payload) = payload.get(..pkt.payload_len) else {
            return;
        };

        let rx_queue = self.udp_ports.lock().get(&pkt.local_port).cloned();
        if let Some(rx_queue) = rx_queue {
            rx_queue.handle_rx(pkt.remote, payload);
        }
    }

    pub fn handle_timeouts(&self, now: MonoTime) {
        self.tcp_flows.lock().poll(now);
        self.garbage_collect();
    }

    fn garbage_collect(&self) {
        // TODO: This might take some time. Optimize this.
        let mut flows = self.tcp_flows.lock();
        while let Some(rule) = flows.pop_closed() {
            if let Err(error) = self.net.unbind(&rule) {
                trace!("failed to unbind flow: {:?}", error);
            }
        }
    }
}
