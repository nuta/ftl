use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem::offset_of;
use core::net::Ipv4Addr;
use core::net::SocketAddrV4;

use ftl::net::Nic;
use ftl::poll::Poll;
use ftl::time::MonoTime;
use ftl::trace;
use ftl_types::error::ErrorCode;
use ftl_types::nic::IP_PROTO_TCP;
use ftl_types::nic::IP_PROTO_UDP;
use ftl_types::nic::PROTO_ANY;
use ftl_types::nic::PROTO_IPV4;
use ftl_types::nic::PROTO_TCP;
use ftl_types::nic::PROTO_UDP;
use ftl_types::nic::RouteRule;
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
pub const INIT_PORT_MIN: u16 = 49152;
pub const INIT_PORT_MAX: u16 = 50175;
pub const ZONE_PORT_LEN: u16 = 1024;
const ZONE_PORT_START: u16 = 10000;

fn alloc_buf(len: usize) -> Result<Vec<u8>, ErrorCode> {
    let mut buf = Vec::new();
    buf.try_reserve_exact(len)
        .map_err(|_| ErrorCode::OutOfMemory)?;
    buf.resize(len, 0);
    Ok(buf)
}

/// Creates a virtual NIC, and route all TX packets to `pnic`.
fn create_nic(pnic: &Nic) -> Result<Nic, ErrorCode> {
    let nic = Nic::create_virtual()?;

    // Route all packets from the virtual NIC to `pnic`. This is so-called
    // "default route".
    let rule = RouteRule::new(PROTO_ANY, 1, u16::MAX);
    nic.bind(&rule, pnic)?;

    Ok(nic)
}

/// Creates a virtual NIC for the init LX.
///
/// - Route all TX packets to `pnic`.
/// - Route RX packets with destination port in [port_min, port_max] received
///   by `pnic`, to the new virtual NIC.
pub fn create_init_nic(pnic: &Nic, port_min: u16, port_max: u16) -> Result<Nic, ErrorCode> {
    let nic = create_nic(pnic)?;

    // Route all [port_min, port_max] destinated packets to `pnic`. This is for
    // ephemeral ports.
    let rule = RouteRule::new(PROTO_ANY, port_min, port_max);

    pnic.bind(&rule, &nic)?;

    Ok(nic)
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

pub struct BindGuard {
    ip: Ipv4Addr,
    pnic: Option<Arc<Nic>>,
    rule: RouteRule,
}

impl BindGuard {
    pub fn port(&self) -> u16 {
        self.rule.port_min
    }

    pub fn accepts(&self, ip: u32) -> bool {
        self.ip.is_unspecified() || self.ip.to_bits() == ip
    }
}

impl Drop for BindGuard {
    fn drop(&mut self) {
        if let Some(pnic) = &self.pnic {
            if let Err(error) = pnic.unbind(&self.rule) {
                trace!("failed to unbind port {}: {:?}", self.port(), error);
            }
        }
    }
}

struct Mutable {
    tcp_conns: HashMap<TcpConnKey, Arc<TcpConn>>,
    tcp_listeners: HashMap<u16, Arc<TcpListener>>,
    udp_ports: HashMap<u16, Arc<RxQueue>>,
    last_alloced_port: u16,
}

pub struct TcpIp {
    nic: Nic,
    pnic: Option<Arc<Nic>>,
    port_min: u16,
    port_max: u16,
    mutable: SpinLock<Mutable>,
}

impl TcpIp {
    pub fn new(nic: Nic, pnic: Option<Arc<Nic>>, port_min: u16, port_max: u16) -> Arc<Self> {
        Arc::new(Self {
            nic,
            pnic,
            port_min,
            port_max,
            mutable: SpinLock::new(Mutable {
                tcp_conns: HashMap::new(),
                tcp_listeners: HashMap::new(),
                udp_ports: HashMap::new(),
                last_alloced_port: port_min,
            }),
        })
    }

    /// Creates a virtual NIC for a child LX.
    pub fn create_parent_nic(&self, port_len: u16) -> Result<(Nic, u16, u16), ErrorCode> {
        let Some(pnic) = &self.pnic else {
            return Err(ErrorCode::NotAllowed);
        };

        let nic = create_nic(pnic)?;

        // Look for a free ephemeral port range.
        let mut port_start = ZONE_PORT_START;
        while port_start + port_len <= INIT_PORT_MIN {
            let port_max = port_start + port_len - 1;
            let rule = RouteRule::new(PROTO_ANY, port_start, port_max);
            match pnic.bind(&rule, &nic) {
                Ok(()) => return Ok((nic, port_start, port_max)),
                Err(ErrorCode::AlreadyExists) => port_start += port_len,
                Err(error) => return Err(error),
            }
        }

        Err(ErrorCode::OutOfBounds)
    }

    pub fn setup_zone_nic(
        &self,
        nic: &Nic,
        host_addr: SocketAddrV4,
        child_ip: Ipv4Addr,
    ) -> Result<(), ErrorCode> {
        // Route: child -> ours.
        let peer_rule = RouteRule {
            protocols: PROTO_ANY,
            prefix_len: 32,
            port_min: host_addr.port(),
            port_max: host_addr.port(),
            ip: host_addr.ip().to_bits(),
            src_ip: child_ip.to_bits(),
        };
        nic.bind(&peer_rule, &self.nic)?;

        // Route: ours -> child.
        let our_rule = RouteRule {
            protocols: PROTO_ANY,
            prefix_len: 32,
            port_min: 1,
            port_max: u16::MAX,
            ip: child_ip.to_bits(),
            src_ip: host_addr.ip().to_bits(),
        };
        self.nic.bind(&our_rule, nic)?;

        Ok(())
    }

    pub fn subscribe(&self, poll: &Poll, cookie: usize) -> Result<(), ErrorCode> {
        self.nic.subscribe(poll, cookie)
    }

    fn owns_port(&self, port: u16) -> bool {
        self.port_min <= port && port <= self.port_max
    }

    pub fn bind_port(
        &self,
        protocols: u8,
        ip: Ipv4Addr,
        port: u16,
    ) -> Result<Arc<BindGuard>, Errno> {
        let rule = RouteRule::new(protocols, port, port);
        if !ip.is_unspecified() || self.owns_port(port) {
            return Ok(Arc::new(BindGuard {
                ip,
                pnic: None,
                rule,
            }));
        }

        let Some(pnic) = &self.pnic else {
            return Err(Errno::EACCES);
        };

        match pnic.bind(&rule, &self.nic) {
            Ok(()) => {}
            Err(ErrorCode::AlreadyExists) => return Err(Errno::EADDRINUSE),
            Err(error) => return Err(error.into()),
        }
        Ok(Arc::new(BindGuard {
            ip,
            pnic: Some(pnic.clone()),
            rule,
        }))
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

    pub fn add_tcp_listener(&self, port: u16, listener: Arc<TcpListener>) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if mutable.tcp_listeners.contains_key(&port) {
            return Err(ErrorCode::AlreadyExists);
        }

        mutable.tcp_listeners.insert(port, listener);
        Ok(())
    }

    pub fn has_tcp_listener(&self, port: u16) -> bool {
        self.mutable.lock().tcp_listeners.contains_key(&port)
    }

    fn alloc_port(
        &self,
        mutable: &mut Mutable,
        protocols: u8,
        ip: Ipv4Addr,
        in_use: impl Fn(&Mutable, u16) -> bool,
    ) -> Result<Arc<BindGuard>, Errno> {
        let num_ports = self.port_max - self.port_min + 1;
        let start = mutable.last_alloced_port - self.port_min + 1;
        for i in 0..num_ports {
            let port = self.port_min + (start + i) % num_ports;
            if !in_use(mutable, port) {
                mutable.last_alloced_port = port;
                return self.bind_port(protocols, ip, port);
            }
        }

        Err(Errno::EADDRNOTAVAIL)
    }

    pub fn alloc_tcp_port(&self, ip: Ipv4Addr) -> Result<Arc<BindGuard>, Errno> {
        let mut mutable = self.mutable.lock();
        self.alloc_port(&mut mutable, PROTO_IPV4 | PROTO_TCP, ip, |mutable, port| {
            mutable.tcp_listeners.contains_key(&port)
        })
    }

    pub fn bind_udp(
        &self,
        local_port: Option<u16>,
        rx_queue: Arc<RxQueue>,
    ) -> Result<Arc<BindGuard>, Errno> {
        let mut mutable = self.mutable.lock();
        let bind_guard = match local_port {
            Some(port) if port != 0 => {
                if mutable.udp_ports.contains_key(&port) {
                    return Err(Errno::EADDRINUSE);
                }

                self.bind_port(PROTO_IPV4 | PROTO_UDP, Ipv4Addr::UNSPECIFIED, port)?
            }
            _ => {
                self.alloc_port(
                    &mut mutable,
                    PROTO_IPV4 | PROTO_UDP,
                    Ipv4Addr::UNSPECIFIED,
                    |mutable, port| mutable.udp_ports.contains_key(&port),
                )?
            }
        };
        mutable.udp_ports.insert(bind_guard.port(), rx_queue);
        Ok(bind_guard)
    }

    pub fn unbind_udp(&self, port: u16) {
        self.mutable.lock().udp_ports.remove(&port);
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
        bind_guard: Option<Arc<BindGuard>>,
        wait_queue: Arc<WaitQueue>,
    ) -> Result<Arc<TcpConn>, Errno> {
        if remote.ip == 0 || remote.port == 0 {
            return Err(Errno::EINVAL);
        }

        let mut iss = [0; 4];
        crate::random::read(&mut iss)?;

        let mut mutable = self.mutable.lock();
        let bind_guard = match bind_guard {
            Some(bind_guard) => {
                let key = TcpConnKey::new(bind_guard.port(), remote);
                if mutable.tcp_conns.contains_key(&key) {
                    return Err(Errno::EADDRINUSE);
                }

                bind_guard
            }
            None => {
                self.alloc_port(
                    &mut mutable,
                    PROTO_IPV4 | PROTO_TCP,
                    Ipv4Addr::UNSPECIFIED,
                    |mutable, port| {
                        mutable
                            .tcp_conns
                            .contains_key(&TcpConnKey::new(port, remote))
                    },
                )?
            }
        };

        let local_port = bind_guard.port();
        let conn = TcpConn::new_active(
            self.clone(),
            wait_queue,
            remote,
            u32::from_ne_bytes(iss),
            bind_guard,
        )?;

        // Register the flow.
        mutable
            .tcp_conns
            .insert(TcpConnKey::new(local_port, remote), conn.clone());
        drop(mutable);

        // Start handshake.
        conn.start_active();
        Ok(conn)
    }

    pub fn connect_tcp_to(self: &Arc<Self>, addr: SocketAddrV4) -> Result<Arc<TcpConn>, Errno> {
        let remote = Endpoint {
            ip: addr.ip().to_bits(),
            port: addr.port(),
        };
        let wait_queue = Arc::new(WaitQueue::new()?);
        self.connect_tcp(remote, None, wait_queue)
    }

    pub fn add_tcp_conn(&self, conn: Arc<TcpConn>, pkt: &TcpPacketInfo) -> Result<(), ErrorCode> {
        let remote = Endpoint {
            ip: pkt.remote_ip,
            port: pkt.remote_port,
        };

        let mut mutable = self.mutable.lock();
        if mutable
            .tcp_conns
            .contains_key(&TcpConnKey::new(pkt.local_port, remote))
        {
            return Err(ErrorCode::AlreadyExists);
        }

        mutable
            .tcp_conns
            .insert(TcpConnKey::new(pkt.local_port, remote), conn);
        Ok(())
    }

    pub fn remove_tcp_conn(&self, local_port: u16, remote: Endpoint, conn: &TcpConn) {
        let mut mutable = self.mutable.lock();
        let key = TcpConnKey::new(local_port, remote);
        if let Some(registered) = mutable.tcp_conns.get(&key)
            && core::ptr::eq(Arc::as_ptr(registered), conn)
        {
            mutable.tcp_conns.remove(&key);
        }
    }

    pub fn unbind_tcp_listener(&self, port: u16) {
        self.mutable.lock().tcp_listeners.remove(&port);
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
                Err(error) if error == ErrorCode::OutOfBounds => {
                    trace!("dropped an oversized network packet");
                    continue;
                }
                Err(_) => panic!("failed to receive a network packet"),
            };

            let payload = &payload[..payload_len];
            match header[offset_of!(Ipv4Header, protocol)] {
                IP_PROTO_TCP => self.handle_tcp_rx(&header, payload),
                IP_PROTO_UDP => self.handle_udp_rx(&header, payload),
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
        let listener = mutable
            .tcp_listeners
            .get(&pkt.local_port)
            .filter(|listener| listener.accepts(pkt.local_ip))
            .cloned();
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
        let mut conns = Vec::new();
        for conn in self.mutable.lock().tcp_conns.values() {
            conns.push(conn.clone());
        }

        for conn in conns {
            conn.poll(now);
        }
    }
}
