use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec::Vec;
use core::num::NonZeroU16;
use core::num::NonZeroU32;

use ftl::net::Net;
use ftl::poll::Poll;
use ftl::trace;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::net::ETHTYPE_IPV4;
use ftl_types::net::FiveTuple;
use ftl_types::net::IPPROTO_TCP;
use ftl_types::net::Rule;
use ftl_utils::spinlock::SpinLock;
use hashbrown::HashMap;

use super::tcp::Endpoint;
use super::tcp::HeaderBuilder;
use super::tcp::Segment;
use super::tcp::TcpConn;
use super::tcp::TcpListener;
use super::tcp::TcpPacketInfo;

pub struct Io {
    net: Net,
}

impl Io {
    fn new(net: Net) -> Self {
        Self { net }
    }

    fn recv_payload(&self, payload: &mut [u8]) -> Result<(), ErrorCode> {
        self.net.recv(payload)
    }

    fn drop_packet(&self) {
        self.net.drop().expect("failed to drop a network packet");
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
        let local_port = NonZeroU16::new(port).ok_or(ErrorCode::INVALID_ARG)?;
        let rule = Rule::new(
            ETHTYPE_IPV4,
            IPPROTO_TCP,
            None,
            Some(local_port),
            None,
            None,
        );
        self.net.bind(&rule, 0 /* cookie is unused */)
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

pub struct ListenerIo<'a> {
    io: &'a Io,
    flows: &'a SpinLock<FlowTable>,
}

impl<'a> ListenerIo<'a> {
    pub fn new(io: &'a Io, flows: &'a SpinLock<FlowTable>) -> Self {
        Self { io, flows }
    }

    pub fn add_connection(&self, conn: Arc<TcpConn>, pkt: &TcpPacketInfo) -> Result<(), ErrorCode> {
        let local_ip = NonZeroU32::new(pkt.local_ip).ok_or(ErrorCode::INVALID_ARG)?;
        let local_port = NonZeroU16::new(pkt.local_port).ok_or(ErrorCode::INVALID_ARG)?;
        let remote_ip = NonZeroU32::new(pkt.remote_ip).ok_or(ErrorCode::INVALID_ARG)?;
        let remote_port = NonZeroU16::new(pkt.remote_port).ok_or(ErrorCode::INVALID_ARG)?;
        let rule = Rule::new(
            ETHTYPE_IPV4,
            IPPROTO_TCP,
            Some(local_ip),
            Some(local_port),
            Some(remote_ip),
            Some(remote_port),
        );

        self.io.net.bind(&rule, 0 /* cookie is unused */)?;
        let mut flows = self.flows.lock();
        flows.flows.insert(pkt.five_tuple(), Flow { conn, rule });
        Ok(())
    }
}

pub struct RecvGuard<'a> {
    io: &'a Io,
    consumed: bool,
}

impl<'a> RecvGuard<'a> {
    pub fn new(io: &'a Io) -> Self {
        Self {
            io,
            consumed: false,
        }
    }

    pub fn recv(mut self, buf: &mut [u8]) -> Result<(), ErrorCode> {
        self.io.recv_payload(buf)?;
        self.consumed = true;
        Ok(())
    }
}

impl<'a> Drop for RecvGuard<'a> {
    fn drop(&mut self) {
        if !self.consumed {
            self.io.drop_packet();
        }
    }
}

struct Flow {
    conn: Arc<TcpConn>,
    rule: Rule,
}

/// Maps transport-layer five-tuples to their state.
pub struct FlowTable {
    flows: HashMap<FiveTuple, Flow>,
}

impl FlowTable {
    fn new() -> Self {
        Self {
            flows: HashMap::new(),
        }
    }

    fn lookup(&self, pkt: &TcpPacketInfo) -> Option<Arc<TcpConn>> {
        self.flows
            .get(&pkt.five_tuple())
            .map(|flow| flow.conn.clone())
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
    listeners: Vec<Weak<TcpListener>>,
}

impl ListenerTable {
    fn new() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }

    fn add(&mut self, listener: &Arc<TcpListener>) {
        self.listeners
            .retain(|listener| listener.strong_count() > 0);
        self.listeners.push(Arc::downgrade(listener));
    }

    fn lookup(&self, pkt: &TcpPacketInfo) -> Option<Arc<TcpListener>> {
        self.listeners
            .iter()
            .filter_map(Weak::upgrade)
            .find(|listener| listener.matches(pkt) && listener.can_accept())
    }
}

pub struct TcpIp {
    io: Arc<Io>,
    listeners: SpinLock<ListenerTable>,
    flows: SpinLock<FlowTable>,
}

impl TcpIp {
    pub fn new(net: Net) -> Arc<Self> {
        Arc::new(Self {
            io: Arc::new(Io::new(net)),
            listeners: SpinLock::new(ListenerTable::new()),
            flows: SpinLock::new(FlowTable::new()),
        })
    }

    pub fn id(&self) -> HandleId {
        self.io.net.id()
    }

    pub fn subscribe(&self, poll: &Poll) -> Result<(), ErrorCode> {
        self.io.net.subscribe(poll)
    }

    pub fn create_listener(self: &Arc<Self>) -> Result<Arc<TcpListener>, ErrorCode> {
        let listener = TcpListener::new(self.io.clone())?;
        self.listeners.lock().add(&listener);
        Ok(listener)
    }

    pub fn handle_rx(&self) {
        let mut header = [0u8; 128];
        loop {
            // Read the packet header.
            match self.io.net.peek(&mut header) {
                Ok(_) => {}
                Err(error) if error == ErrorCode::EMPTY => return,
                Err(_) => panic!("failed to peek at a network packet"),
            }

            // Parse the header.
            let recv_guard = RecvGuard::new(&self.io);
            let pkt = TcpPacketInfo::parse(&header);

            // Lookup the flow or listener.
            if let Some(conn) = self.flows.lock().lookup(&pkt) {
                conn.handle_rx(&pkt, recv_guard);
            } else if let Some(listener) = self.listeners.lock().lookup(&pkt) {
                let listener_io = ListenerIo::new(&self.io, &self.flows);
                listener.handle_rx(&pkt, recv_guard, listener_io);
            }

            // Garbage-collect closed flows.
            // TODO: This might take some time. Optimize this.
            let mut flows = self.flows.lock();
            while let Some(rule) = flows.pop_closed() {
                if let Err(error) = self.io.net.unbind(&rule) {
                    trace!("failed to unbind flow: {:?}", error);
                }
            }
        }
    }
}
