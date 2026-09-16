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

const RX_HEADER_LEN: usize = 128;
const RX_PAYLOAD_LEN: usize = 2048;

fn alloc_buf(len: usize) -> Result<Vec<u8>, ErrorCode> {
    let mut buf = Vec::new();
    buf.try_reserve_exact(len)
        .map_err(|_| ErrorCode::OutOfMemory)?;
    buf.resize(len, 0);
    Ok(buf)
}

pub struct Io {
    net: Net,
}

impl Io {
    fn new(net: Net) -> Self {
        Self { net }
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
        let local_ip = NonZeroU32::new(pkt.local_ip).ok_or(ErrorCode::InvalidArg)?;
        let local_port = NonZeroU16::new(pkt.local_port).ok_or(ErrorCode::InvalidArg)?;
        let remote_ip = NonZeroU32::new(pkt.remote_ip).ok_or(ErrorCode::InvalidArg)?;
        let remote_port = NonZeroU16::new(pkt.remote_port).ok_or(ErrorCode::InvalidArg)?;
        let rule = Rule::new(
            ETHTYPE_IPV4,
            IPPROTO_TCP,
            Some(local_ip),
            Some(local_port),
            Some(remote_ip),
            Some(remote_port),
        );

        self.io.net.bind(&rule)?;
        let mut flows = self.flows.lock();
        flows.flows.insert(pkt.five_tuple(), Flow { conn, rule });
        Ok(())
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
            if let Some(conn) = self.flows.lock().lookup(&pkt) {
                conn.handle_rx(&pkt, payload);
            } else if let Some(listener) = self.listeners.lock().lookup(&pkt) {
                let listener_io = ListenerIo::new(&self.io, &self.flows);
                listener.handle_rx(&pkt, payload, listener_io);
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
