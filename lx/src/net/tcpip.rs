use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem::offset_of;

use ftl::net::Nic;
use ftl::poll::Poll;
use ftl::time::MonoTime;
use ftl::trace;
use ftl_types::error::ErrorCode;
use ftl_types::net::IPPROTO_TCP;
use ftl_types::net::IPPROTO_UDP;
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
pub const INIT_PORT_START: u16 = 49152;
pub const INIT_PORT_LEN: u16 = 16384;
pub const ZONE_PORT_LEN: u16 = 1024;

fn alloc_buf(len: usize) -> Result<Vec<u8>, ErrorCode> {
    let mut buf = Vec::new();
    buf.try_reserve_exact(len)
        .map_err(|_| ErrorCode::OutOfMemory)?;
    buf.resize(len, 0);
    Ok(buf)
}

pub struct TcpIp {
    nic: Nic,
    pnic: Option<Nic>,
    port_start: u16,
    port_len: u16,
    ports: SpinLock<HashMap<u16, usize>>,
    tcp_flows: SpinLock<TcpFlows>,
    tcp_listeners: SpinLock<HashMap<u16, Arc<TcpListener>>>,
    udp_ports: SpinLock<HashMap<u16, Arc<RxQueue>>>,
}

impl TcpIp {
    pub fn new(nic: Nic, pnic: Option<Nic>, port_start: u16, port_len: u16) -> Arc<Self> {
        Arc::new(Self {
            nic,
            pnic,
            port_start,
            port_len,
            ports: SpinLock::new(HashMap::new()),
            tcp_flows: SpinLock::new(TcpFlows::new()),
            tcp_listeners: SpinLock::new(HashMap::new()),
            udp_ports: SpinLock::new(HashMap::new()),
        })
    }

    pub fn create_uplink(&self, port_len: u16) -> Result<(Nic, u16), ErrorCode> {
        let Some(pnic) = &self.pnic else {
            return Err(ErrorCode::NotAllowed);
        };

        let nic = Nic::create_virtual()?;
        let port_start = pnic.bind(0, port_len, &nic)?;
        nic.bind(1, u16::MAX, pnic)?;
        Ok((nic, port_start))
    }

    pub fn subscribe(&self, poll: &Poll, cookie: usize) -> Result<(), ErrorCode> {
        self.nic.subscribe(poll, cookie)
    }

    fn owns_port(&self, port: u16) -> bool {
        let start = self.port_start as u32;
        let port = port as u32;
        start <= port && port < start + self.port_len as u32
    }

    fn acquire_port(&self, port: u16) -> Result<(), ErrorCode> {
        if port == 0 {
            return Err(ErrorCode::InvalidArg);
        }

        let mut ports = self.ports.lock();
        let count = ports.get(&port).copied().unwrap_or(0);
        if count == 0 && !self.owns_port(port) {
            let Some(pnic) = &self.pnic else {
                return Err(ErrorCode::NotAllowed);
            };
            pnic.bind(port, 1, &self.nic)?;
        }

        ports.insert(port, count + 1);
        Ok(())
    }

    fn release_port(&self, port: u16) {
        let mut ports = self.ports.lock();
        let count = ports.get(&port).copied().unwrap_or(0);
        if count > 1 {
            ports.insert(port, count - 1);
            return;
        }

        ports.remove(&port);
        if !self.owns_port(port) {
            if let Some(pnic) = &self.pnic {
                let _ = pnic.unbind(port, 1);
            }
        }
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
        self.nic.send(&header, segment.payload)
    }

    pub fn bind_tcp_listener(
        &self,
        port: u16,
        listener: Arc<TcpListener>,
    ) -> Result<(), ErrorCode> {
        let mut listeners = self.tcp_listeners.lock();
        if listeners.contains_key(&port) {
            return Err(ErrorCode::AlreadyExists);
        }

        self.acquire_port(port)?;
        listeners.insert(port, listener);
        Ok(())
    }

    fn bind_tcp_conn(&self, local_port: Option<u16>, remote: Endpoint) -> Result<u16, Errno> {
        if remote.ip == 0 || remote.port == 0 {
            return Err(Errno::EINVAL);
        }

        // Bind to a specific port.
        if let Some(port) = local_port
            && port != 0
        {
            if self.tcp_flows.lock().contains(port, remote) {
                return Err(Errno::EADDRINUSE);
            }

            return match self.acquire_port(port) {
                Ok(()) => Ok(port),
                Err(ErrorCode::AlreadyExists) => Err(Errno::EADDRINUSE),
                Err(error) => Err(error.into()),
            };
        }

        // Start scanning from a random port.
        let mut seed = [0; 2];
        crate::random::read(&mut seed)?;
        let num_ports = self.port_len;
        let start = u16::from_ne_bytes(seed) % num_ports;

        for i in 0..num_ports {
            let port = self.port_start + (start + i) % num_ports;
            if !self.tcp_flows.lock().contains(port, remote) {
                self.acquire_port(port)?;
                return Ok(port);
            }
        }

        Err(Errno::EADDRNOTAVAIL)
    }

    pub fn bind_udp(&self, local_port: Option<u16>, rx_queue: Arc<RxQueue>) -> Result<u16, Errno> {
        let mut udp_ports = self.udp_ports.lock();
        if let Some(port) = local_port
            && port != 0
        {
            if udp_ports.contains_key(&port) {
                return Err(Errno::EADDRINUSE);
            }

            return match self.acquire_port(port) {
                Ok(()) => {
                    udp_ports.insert(port, rx_queue);
                    Ok(port)
                }
                Err(ErrorCode::AlreadyExists) => Err(Errno::EADDRINUSE),
                Err(error) => Err(error.into()),
            };
        }

        // Assign an ephemeral port.
        for i in 0..self.port_len {
            let port = self.port_start + i;
            if !udp_ports.contains_key(&port) {
                self.acquire_port(port)?;
                udp_ports.insert(port, rx_queue);
                return Ok(port);
            }
        }

        Err(Errno::EADDRINUSE)
    }

    pub fn unbind_udp(&self, port: u16) {
        if self.udp_ports.lock().remove(&port).is_some() {
            self.release_port(port);
        }
    }

    pub fn send_udp(
        &self,
        remote: Endpoint,
        local_port: u16,
        payload: &[u8],
    ) -> Result<(), ErrorCode> {
        let builder = UdpHeaderBuilder::new();
        let header = builder.build(remote, local_port, payload.len());
        self.nic.send(&header, payload)
    }

    pub fn connect_tcp(
        self: &Arc<Self>,
        remote: Endpoint,
        local_port: Option<u16>,
        wait_queue: Arc<WaitQueue>,
    ) -> Result<Arc<TcpConn>, Errno> {
        let mut iss = [0; 4];
        crate::random::read(&mut iss)?;

        let local_port = self.bind_tcp_conn(local_port, remote)?;
        let conn = match TcpConn::new_active(
            self.clone(),
            wait_queue,
            remote,
            local_port,
            u32::from_ne_bytes(iss),
        ) {
            Ok(conn) => conn,
            Err(error) => {
                self.release_port(local_port);
                return Err(error.into());
            }
        };

        // Register the flow.
        self.tcp_flows
            .lock()
            .insert(local_port, remote, conn.clone());

        // Start handshake.
        conn.start_active();
        Ok(conn)
    }

    pub fn add_tcp_conn(&self, conn: Arc<TcpConn>, pkt: &TcpPacketInfo) -> Result<(), ErrorCode> {
        let remote = Endpoint {
            ip: pkt.remote_ip,
            port: pkt.remote_port,
        };

        let mut flows = self.tcp_flows.lock();
        if flows.contains(pkt.local_port, remote) {
            return Err(ErrorCode::AlreadyExists);
        }

        self.acquire_port(pkt.local_port)?;
        flows.insert(pkt.local_port, remote, conn);
        Ok(())
    }

    pub fn unbind_tcp_listener(&self, port: u16) {
        if self.tcp_listeners.lock().remove(&port).is_some() {
            self.release_port(port);
        }
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
            let payload_len = match self.nic.recv(&mut header, &mut payload) {
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
        while let Some(port) = flows.pop_closed() {
            self.release_port(port);
        }
    }
}
