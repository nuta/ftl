use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

use ftl::info;
use ftl::syscall::net_peek;
use ftl::syscall::net_recv;
use ftl::syscall::net_send;
use ftl::syscall::poll_wait;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::net::NetRxInfo;
use ftl_utils::spinlock::SpinLock;

use super::tcp_conn::TcpConnection;
use super::tcp_listener::TcpListener;
use super::tcp_packet::Endpoint;
use super::tcp_packet::Segment;
use super::tcp_packet::build_header;

pub struct NetworkService {
    poll: HandleId,
    net: HandleId,
    listener: SpinLock<Option<Arc<TcpListener>>>,
    connections: SpinLock<Vec<Arc<TcpConnection>>>,
}

impl NetworkService {
    pub fn new(poll: HandleId, net: HandleId) -> Arc<Self> {
        Arc::new(Self {
            poll,
            net,
            listener: SpinLock::new(None),
            connections: SpinLock::new(Vec::new()),
        })
    }

    pub fn create_listener(self: &Arc<Self>, poll: HandleId) -> Arc<TcpListener> {
        let listener = TcpListener::new(Arc::downgrade(self), poll);
        *self.listener.lock() = Some(listener.clone());
        listener
    }

    pub(super) fn add_connection(&self, connection: Arc<TcpConnection>) {
        self.connections.lock().push(connection);
    }

    fn recv(&self) -> Result<Option<(NetRxInfo, Vec<u8>)>, ErrorCode> {
        let mut info = NetRxInfo::empty();
        let token = match net_peek(self.net, &mut info) {
            Ok(token) => token,
            Err(error) if error == ErrorCode::EMPTY => return Ok(None),
            Err(error) => return Err(error),
        };

        let mut payload = vec![0; info.payload_len as usize];
        net_recv(self.net, token, &mut payload)?;
        Ok(Some((info, payload)))
    }

    fn drain(&self) {
        loop {
            let packet = self.recv().expect("failed to receive a network packet");
            let Some((info, payload)) = packet else {
                return;
            };
            self.handle_packet(&info, &payload);
        }
    }

    pub fn run(&self) -> ! {
        info!("tcpip: network event loop is ready");
        loop {
            poll_wait(self.poll).expect("network poll failed");
            self.drain();
        }
    }

    fn find_connection(&self, info: &NetRxInfo) -> Option<Arc<TcpConnection>> {
        let connections = self.connections.lock();
        for connection in connections.iter() {
            if connection.matches(info) {
                return Some(connection.clone());
            }
        }
        None
    }

    fn listener(&self) -> Option<Arc<TcpListener>> {
        let listener = self.listener.lock();
        match &*listener {
            Some(listener) => Some(listener.clone()),
            None => None,
        }
    }

    fn remove_closed_connections(&self) {
        let mut connections = self.connections.lock();
        let mut index = 0;
        while index < connections.len() {
            if connections[index].is_closed() {
                connections.remove(index);
            } else {
                index += 1;
            }
        }
    }

    fn handle_packet(&self, info: &NetRxInfo, payload: &[u8]) {
        if let Some(connection) = self.find_connection(info) {
            connection.handle_packet(info, payload);
            self.remove_closed_connections();
            return;
        }

        let Some(listener) = self.listener() else {
            return;
        };
        if !listener.accepts(info.local_port) {
            return;
        }
        listener.handle_packet(info, payload);
    }

    pub(super) fn send_segment(
        &self,
        remote: Endpoint,
        local_port: u16,
        segment: Segment,
    ) -> Result<(), ErrorCode> {
        let header = build_header(remote, local_port, &segment);
        net_send(self.net, &header, segment.payload)
    }
}
