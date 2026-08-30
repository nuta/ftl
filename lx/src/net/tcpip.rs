use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::num::NonZeroU16;
use core::num::NonZeroU32;

use ftl::syscall::net_bind;
use ftl::syscall::net_drop;
use ftl::syscall::net_peek;
use ftl::syscall::net_recv;
use ftl::syscall::net_send;
use ftl::syscall::net_unbind;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::net::NET_MAX_HEADER_LEN;
use ftl_types::net::NetMatch;
use ftl_types::net::NetRxMeta;
use ftl_utils::spinlock::SpinLock;

use super::tcp::Endpoint;
use super::tcp::Segment;
use super::tcp::TcpConnection;
use super::tcp::TcpListener;
use super::tcp::TcpSegmentMeta;
use super::tcp::build_header;
use super::tcp::parse_received;

struct ConnectionEntry {
    connection: Arc<TcpConnection>,
    selector: NetMatch,
}

struct Connections {
    next_cookie: u64,
    entries: BTreeMap<u64, ConnectionEntry>,
}

pub struct TcpIp {
    net: HandleId,
    listener_cookie: u64,
    listener: SpinLock<Option<Arc<TcpListener>>>,
    connections: SpinLock<Connections>,
    rx_header: SpinLock<Vec<u8>>,
}

impl TcpIp {
    pub fn new(net: HandleId, listener_cookie: u64) -> Arc<Self> {
        Arc::new(Self {
            net,
            listener_cookie,
            listener: SpinLock::new(None),
            connections: SpinLock::new(Connections {
                next_cookie: listener_cookie.wrapping_add(1).max(1),
                entries: BTreeMap::new(),
            }),
            rx_header: SpinLock::new(vec![0; NET_MAX_HEADER_LEN]),
        })
    }

    pub fn create_listener(self: &Arc<Self>, poll: HandleId) -> Arc<TcpListener> {
        let listener = TcpListener::new(Arc::downgrade(self), poll);
        *self.listener.lock() = Some(listener.clone());
        listener
    }

    pub fn add_connection(
        &self,
        connection: Arc<TcpConnection>,
        info: &TcpSegmentMeta,
    ) -> Result<(), ErrorCode> {
        let local_ip = NonZeroU32::new(info.local_ip).ok_or(ErrorCode::INVALID_ARG)?;
        let local_port = NonZeroU16::new(info.local_port).ok_or(ErrorCode::INVALID_ARG)?;
        let remote_ip = NonZeroU32::new(info.remote_ip).ok_or(ErrorCode::INVALID_ARG)?;
        let remote_port = NonZeroU16::new(info.remote_port).ok_or(ErrorCode::INVALID_ARG)?;
        let selector = NetMatch::tcp_ipv4_flow(local_ip, local_port, remote_ip, remote_port);
        let mut connections = self.connections.lock();
        while connections.next_cookie == self.listener_cookie
            || connections.entries.contains_key(&connections.next_cookie)
        {
            connections.next_cookie = connections.next_cookie.wrapping_add(1).max(1);
        }
        let cookie = connections.next_cookie;
        connections.next_cookie = connections.next_cookie.wrapping_add(1).max(1);
        net_bind(self.net, &selector, cookie)?;
        connections.entries.insert(
            cookie,
            ConnectionEntry {
                connection,
                selector,
            },
        );
        Ok(())
    }

    fn drain(&self) {
        let mut header = self.rx_header.lock();
        loop {
            let mut rx = NetRxMeta::empty();
            match net_peek(self.net, &mut header, &mut rx) {
                Ok(()) => {}
                Err(error) if error == ErrorCode::EMPTY => return,
                Err(_) => panic!("failed to peek at a network packet"),
            }
            let Some(info) = parse_received(&header, &rx) else {
                net_drop(self.net).expect("failed to drop a network packet");
                continue;
            };
            self.handle_packet(rx.cookie, &info);
        }
    }

    pub fn handle_event(&self) {
        self.drain();
    }

    fn find_connection(&self, info: &TcpSegmentMeta) -> Option<Arc<TcpConnection>> {
        let connections = self.connections.lock();
        for entry in connections.entries.values() {
            if entry.connection.matches(info) {
                return Some(entry.connection.clone());
            }
        }
        None
    }

    fn connection(&self, cookie: u64) -> Option<Arc<TcpConnection>> {
        self.connections
            .lock()
            .entries
            .get(&cookie)
            .map(|entry| entry.connection.clone())
    }

    fn listener(&self) -> Option<Arc<TcpListener>> {
        let listener = self.listener.lock();
        match &*listener {
            Some(listener) => Some(listener.clone()),
            None => None,
        }
    }

    fn remove_closed_connections(&self) {
        loop {
            let selector = {
                let mut connections = self.connections.lock();
                let mut closed_cookie = None;
                for (cookie, entry) in &connections.entries {
                    if entry.connection.is_closed() {
                        closed_cookie = Some(*cookie);
                        break;
                    }
                }
                let Some(cookie) = closed_cookie else {
                    return;
                };
                connections.entries.remove(&cookie).unwrap().selector
            };
            let _ = net_unbind(self.net, &selector);
        }
    }

    fn handle_packet(&self, cookie: u64, info: &TcpSegmentMeta) {
        let connection = if cookie == self.listener_cookie {
            self.find_connection(info)
        } else {
            self.connection(cookie)
        };
        let consumed = if let Some(connection) = connection {
            connection.handle_packet(info)
        } else if cookie != self.listener_cookie {
            false
        } else if let Some(listener) = self.listener() {
            listener.accepts(info.local_port) && listener.handle_packet(info)
        } else {
            false
        };
        if !consumed {
            self.drop_packet();
        }
        self.remove_closed_connections();
    }

    pub fn recv_payload(&self, payload: &mut [u8]) -> Result<(), ErrorCode> {
        net_recv(self.net, payload)
    }

    fn drop_packet(&self) {
        net_drop(self.net).expect("failed to drop a network packet");
    }

    pub fn send_segment(
        &self,
        remote: Endpoint,
        local_port: u16,
        segment: Segment,
    ) -> Result<(), ErrorCode> {
        let header = build_header(remote, local_port, &segment);
        net_send(self.net, &header, segment.payload)
    }
}
