use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem::offset_of;
use core::num::NonZeroU16;

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

pub fn create_uplink(pnic: &Nic, port_start: u16, port_len: u16) -> Result<(Nic, u16), ErrorCode> {
    let nic = Nic::create_virtual()?;
    let port_start = pnic.bind(port_start, port_len, &nic)?;
    nic.bind(1, u16::MAX, pnic)?;
    Ok((nic, port_start))
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct TcpConnKey {
    local_port: u16,
    remote: Endpoint,
}

impl TcpConnKey {
    fn new(local_port: u16, remote: Endpoint) -> Self {
        Self { local_port, remote }
    }
}

struct Mutable {
    granted_ports: HashMap<u16, usize>,
    tcp_conns: HashMap<TcpConnKey, Arc<TcpConn>>,
    tcp_listeners: HashMap<u16, Arc<TcpListener>>,
    udp_ports: HashMap<u16, Arc<RxQueue>>,
}

pub struct TcpIp {
    nic: Nic,
    pnic: Option<Nic>,
    port_start: u16,
    port_len: u16,
    mutable: SpinLock<Mutable>,
}

impl TcpIp {
    pub fn new(nic: Nic, pnic: Option<Nic>, port_start: u16, port_len: u16) -> Arc<Self> {
        Arc::new(Self {
            nic,
            pnic,
            port_start,
            port_len,
            mutable: SpinLock::new(Mutable {
                granted_ports: HashMap::new(),
                tcp_conns: HashMap::new(),
                tcp_listeners: HashMap::new(),
                udp_ports: HashMap::new(),
            }),
        })
    }

    pub fn create_uplink(&self, port_len: u16) -> Result<(Nic, u16), ErrorCode> {
        let Some(pnic) = &self.pnic else {
            return Err(ErrorCode::NotAllowed);
        };

        create_uplink(pnic, 0, port_len)
    }

    pub fn subscribe(&self, poll: &Poll, cookie: usize) -> Result<(), ErrorCode> {
        self.nic.subscribe(poll, cookie)
    }

    fn owns_port(&self, port: u16) -> bool {
        let start = self.port_start as u32;
        let port = port as u32;
        start <= port && port < start + self.port_len as u32
    }

    fn acquire_port(&self, mutable: &mut Mutable, port: NonZeroU16) -> Result<(), ErrorCode> {
        let port = port.get();
        if self.owns_port(port) {
            return Ok(());
        }

        let count = mutable.granted_ports.get(&port).copied().unwrap_or(0);
        if count == 0 {
            let Some(pnic) = &self.pnic else {
                return Err(ErrorCode::NotAllowed);
            };
            pnic.bind(port, 1, &self.nic)?;
        }

        mutable.granted_ports.insert(port, count + 1);
        Ok(())
    }

    fn release_port(&self, mutable: &mut Mutable, port: u16) {
        if self.owns_port(port) {
            return;
        }

        let count = mutable.granted_ports.get(&port).copied().unwrap_or(0);
        if count > 1 {
            mutable.granted_ports.insert(port, count - 1);
            return;
        }

        mutable.granted_ports.remove(&port);
        if let Some(pnic) = &self.pnic {
            let _ = pnic.unbind(port, 1);
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
        let Some(nonzero_port) = NonZeroU16::new(port) else {
            return Err(ErrorCode::InvalidArg);
        };

        let mut mutable = self.mutable.lock();
        if mutable.tcp_listeners.contains_key(&port) {
            return Err(ErrorCode::AlreadyExists);
        }

        self.acquire_port(&mut mutable, nonzero_port)?;
        mutable.tcp_listeners.insert(port, listener);
        Ok(())
    }

    fn alloc_port(
        &self,
        mutable: &mut Mutable,
        port: Option<u16>,
        in_use: impl Fn(&Mutable, u16) -> bool,
    ) -> Result<u16, Errno> {
        // Bind to a specific port.
        if let Some(port) = port
            && let Some(port) = NonZeroU16::new(port)
        {
            if in_use(mutable, port.get()) {
                return Err(Errno::EADDRINUSE);
            }

            return match self.acquire_port(mutable, port) {
                Ok(()) => Ok(port.get()),
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
            let Some(port) = NonZeroU16::new(self.port_start + (start + i) % num_ports) else {
                continue;
            };
            if !in_use(mutable, port.get()) {
                self.acquire_port(mutable, port)?;
                return Ok(port.get());
            }
        }

        Err(Errno::EADDRNOTAVAIL)
    }

    pub fn bind_udp(&self, local_port: Option<u16>, rx_queue: Arc<RxQueue>) -> Result<u16, Errno> {
        let mut mutable = self.mutable.lock();
        let port = self.alloc_port(&mut mutable, local_port, |mutable, port| {
            mutable.udp_ports.contains_key(&port)
        })?;
        mutable.udp_ports.insert(port, rx_queue);
        Ok(port)
    }

    pub fn unbind_udp(&self, port: u16) {
        let mut mutable = self.mutable.lock();
        if mutable.udp_ports.remove(&port).is_some() {
            self.release_port(&mut mutable, port);
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
        if remote.ip == 0 || remote.port == 0 {
            return Err(Errno::EINVAL);
        }

        let mut iss = [0; 4];
        crate::random::read(&mut iss)?;

        let mut mutable = self.mutable.lock();
        let local_port = self.alloc_port(&mut mutable, local_port, |mutable, port| {
            mutable
                .tcp_conns
                .contains_key(&TcpConnKey::new(port, remote))
        })?;
        let conn = match TcpConn::new_active(
            self.clone(),
            wait_queue,
            remote,
            local_port,
            u32::from_ne_bytes(iss),
        ) {
            Ok(conn) => conn,
            Err(error) => {
                self.release_port(&mut mutable, local_port);
                return Err(error.into());
            }
        };

        // Register the flow.
        mutable
            .tcp_conns
            .insert(TcpConnKey::new(local_port, remote), conn.clone());
        drop(mutable);

        // Start handshake.
        conn.start_active();
        Ok(conn)
    }

    pub fn add_tcp_conn(&self, conn: Arc<TcpConn>, pkt: &TcpPacketInfo) -> Result<(), ErrorCode> {
        let remote = Endpoint {
            ip: pkt.remote_ip,
            port: pkt.remote_port,
        };

        let Some(local_port) = NonZeroU16::new(pkt.local_port) else {
            return Err(ErrorCode::InvalidArg);
        };

        let mut mutable = self.mutable.lock();
        if mutable
            .tcp_conns
            .contains_key(&TcpConnKey::new(pkt.local_port, remote))
        {
            return Err(ErrorCode::AlreadyExists);
        }

        self.acquire_port(&mut mutable, local_port)?;
        mutable
            .tcp_conns
            .insert(TcpConnKey::new(pkt.local_port, remote), conn);
        Ok(())
    }

    pub fn unbind_tcp_listener(&self, port: u16) {
        let mut mutable = self.mutable.lock();
        if mutable.tcp_listeners.remove(&port).is_some() {
            self.release_port(&mut mutable, port);
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
        let remote = Endpoint {
            ip: pkt.remote_ip,
            port: pkt.remote_port,
        };

        let mutable = self.mutable.lock();
        let conn = mutable
            .tcp_conns
            .get(&TcpConnKey::new(pkt.local_port, remote))
            .cloned();
        let listener = mutable.tcp_listeners.get(&pkt.local_port).cloned();
        drop(mutable);

        if let Some(conn) = conn {
            conn.handle_rx(&pkt, payload);
        } else if let Some(listener) = listener {
            listener.handle_rx(&pkt, payload);
        }
    }

    fn handle_udp_rx(&self, header: &[u8], payload: &[u8]) {
        let pkt = UdpPacketInfo::parse(header);

        let Some(payload) = payload.get(..pkt.payload_len) else {
            return;
        };

        let rx_queue = self.mutable.lock().udp_ports.get(&pkt.local_port).cloned();
        if let Some(rx_queue) = rx_queue {
            rx_queue.handle_rx(pkt.remote, payload);
        }
    }

    pub fn handle_timeouts(&self, now: MonoTime) {
        let mut mutable = self.mutable.lock();
        for conn in mutable.tcp_conns.values() {
            conn.poll(now);
        }

        // TODO: This might take some time. Optimize this.
        let mut closed = Vec::new();
        mutable.tcp_conns.retain(|key, conn| {
            if conn.is_closed() {
                closed.push(key.local_port);
                false
            } else {
                true
            }
        });

        for port in closed {
            self.release_port(&mut mutable, port);
        }
    }
}
