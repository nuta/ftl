use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec::Vec;
use core::num::NonZeroU16;
use core::num::NonZeroU32;

use ftl::net::Net;
use ftl::poll::Poll;
use ftl::time::MonoTime;
use ftl::trace;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::net::ETHTYPE_IPV4;
use ftl_types::net::IPPROTO_TCP;
use ftl_types::net::Rule;
use ftl_utils::spinlock::SpinLock;
use hashbrown::HashMap;

use super::tcp::Endpoint;
use super::tcp::HeaderBuilder;
use super::tcp::Segment;
use super::tcp::TcpConn;
use super::tcp::TcpPacketInfo;
use super::tcp::TcpSocket;
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

pub struct Io {
    net: Net,
    flows: SpinLock<FlowTable>,
}

impl Io {
    fn new(net: Net) -> Self {
        Self {
            net,
            flows: SpinLock::new(FlowTable::new()),
        }
    }

    pub fn send_segment(
        &self,
        remote: Endpoint,
        local_port: u16,
        segment: Segment,
    ) -> Result<(), ErrorCode> {
        let header = HeaderBuilder::new().build(remote, local_port, &segment);
        self.net.send(&header, segment.payload)
    }

    pub fn bind_listener(&self, port: u16) -> Result<(), ErrorCode> {
        let local_port = NonZeroU16::new(port).ok_or(ErrorCode::InvalidArg)?;
        let rule = Rule::new(
            ETHTYPE_IPV4,
            IPPROTO_TCP,
            None,
            Some(local_port),
            None,
            None,
        );
        self.net.bind(&rule)
    }

    fn bind_flow(&self, local_port: Option<u16>, remote: Endpoint) -> Result<(u16, Rule), Errno> {
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

    pub fn connect(
        self: &Arc<Self>,
        remote: Endpoint,
        local_port: Option<u16>,
        wait_queue: Arc<WaitQueue>,
    ) -> Result<Arc<TcpConn>, Errno> {
        let mut iss = [0; 4];
        crate::random::read(&mut iss)?;

        let (local_port, rule) = self.bind_flow(local_port, remote)?;
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
        let key = FlowKey {
            local_port,
            remote_ip: remote.ip,
            remote_port: remote.port,
        };
        let flow = Flow {
            conn: conn.clone(),
            rule,
        };
        self.flows.lock().flows.insert(key, flow);

        // Start handshake.
        conn.start_active();
        Ok(conn)
    }

    pub fn add_connection(&self, conn: Arc<TcpConn>, pkt: &TcpPacketInfo) -> Result<(), ErrorCode> {
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
        let key = FlowKey::from_packet(pkt);
        self.flows.lock().flows.insert(key, Flow { conn, rule });
        Ok(())
    }

    pub fn unbind_listener(&self, port: u16) {
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
}

struct Flow {
    conn: Arc<TcpConn>,
    rule: Rule,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct FlowKey {
    local_port: u16,
    remote_ip: u32,
    remote_port: u16,
}

impl FlowKey {
    fn from_packet(pkt: &TcpPacketInfo) -> Self {
        Self {
            local_port: pkt.local_port,
            remote_ip: pkt.remote_ip,
            remote_port: pkt.remote_port,
        }
    }
}

/// Maps flows to their state.
struct FlowTable {
    flows: HashMap<FlowKey, Flow>,
}

impl FlowTable {
    fn new() -> Self {
        Self {
            flows: HashMap::new(),
        }
    }

    fn lookup(&self, pkt: &TcpPacketInfo) -> Option<Arc<TcpConn>> {
        let key = FlowKey::from_packet(pkt);
        self.flows.get(&key).map(|flow| flow.conn.clone())
    }

    fn poll(&self, now: MonoTime) {
        for flow in self.flows.values() {
            flow.conn.poll(now);
        }
    }

    // TODO: Optimize this.
    fn pop_closed(&mut self) -> Option<Rule> {
        let mut to_remove = None;
        for (tuple, flow) in self.flows.iter_mut() {
            if flow.conn.is_closed() {
                to_remove = Some((*tuple, flow.rule));
                break;
            }
        }

        if let Some((tuple, rule)) = to_remove {
            self.flows.remove(&tuple);
            Some(rule)
        } else {
            None
        }
    }
}

/// TCP listeners.
///
/// FIXME: Weak references are used to GC, but we should remove the listener
///        explicitly when it's closed, not lazily. I need some more time to
///        come up with a cleaner solution...
struct ListenerTable {
    listeners: Vec<Weak<TcpSocket>>,
}

impl ListenerTable {
    fn new() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }

    fn add(&mut self, listener: &Arc<TcpSocket>) {
        self.listeners
            .retain(|listener| listener.strong_count() > 0);
        self.listeners.push(Arc::downgrade(listener));
    }

    fn lookup(&self, pkt: &TcpPacketInfo) -> Option<Arc<TcpSocket>> {
        self.listeners
            .iter()
            .filter_map(Weak::upgrade)
            .find(|listener| listener.matches(pkt) && listener.can_accept())
    }
}

pub struct TcpIp {
    io: Arc<Io>,
    listeners: SpinLock<ListenerTable>,
}

impl TcpIp {
    pub fn new(net: Net) -> Arc<Self> {
        Arc::new(Self {
            io: Arc::new(Io::new(net)),
            listeners: SpinLock::new(ListenerTable::new()),
        })
    }

    pub fn id(&self) -> HandleId {
        self.io.net.id()
    }

    pub fn subscribe(&self, poll: &Poll) -> Result<(), ErrorCode> {
        self.io.net.subscribe(poll)
    }

    pub fn create_socket(self: &Arc<Self>) -> Result<Arc<TcpSocket>, ErrorCode> {
        let socket = TcpSocket::new(self.io.clone())?;
        self.listeners.lock().add(&socket);
        Ok(socket)
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
            let payload_len = match self.io.net.recv(&mut header, &mut payload) {
                Ok(len) => len,
                Err(error) if error == ErrorCode::Empty => return,
                Err(_) => panic!("failed to receive a network packet"),
            };

            let pkt = TcpPacketInfo::parse(&header);
            let payload = &payload[..payload_len];

            // Lookup the flow or listener.
            if let Some(conn) = self.io.flows.lock().lookup(&pkt) {
                conn.handle_rx(&pkt, payload);
            } else if let Some(listener) = self.listeners.lock().lookup(&pkt) {
                listener.handle_rx(&pkt, payload);
            }

            self.garbage_collect();
        }
    }

    pub fn handle_timeouts(&self, now: MonoTime) {
        self.io.flows.lock().poll(now);
        self.garbage_collect();
    }

    fn garbage_collect(&self) {
        // TODO: This might take some time. Optimize this.
        let mut flows = self.io.flows.lock();
        while let Some(rule) = flows.pop_closed() {
            if let Err(error) = self.io.net.unbind(&rule) {
                trace!("failed to unbind flow: {:?}", error);
            }
        }
    }
}
