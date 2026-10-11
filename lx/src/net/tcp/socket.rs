use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;

use ftl::trace;
use ftl_types::error::ErrorCode;
use ftl_utils::spinlock::SpinLock;

use super::buffer::TCP_BUFFER_SIZE;
use super::conn::TcpConn;
use super::packet::Endpoint;
use super::packet::Segment;
use super::packet::TcpFlags;
use super::packet::TcpPacketInfo;
use crate::net::tcpip::Io;
use crate::net::tcpip::ListenerIo;
use crate::types::c_int;
use crate::types::c_short;
use crate::types::errno::Errno;
use crate::types::sys::poll::POLLIN;
use crate::types::sys::socket::SO_REUSEADDR;
use crate::types::sys::socket::SOL_SOCKET;
use crate::types::sys::socket::SockAddr;
use crate::vfs::FileLike;
use crate::wait_queue::Sleep;
use crate::wait_queue::WaitQueue;

const INITIAL_SEND_SEQ: u32 = 1234;

#[derive(Clone, Copy)]
struct Handshake {
    remote: Endpoint,
    local_iss: u32,
    remote_rcv_nxt: u32,
    remote_rcv_wnd: u16,
}

struct Listening {
    local_port: u16,
    backlog: usize,
    inflights: Vec<Handshake>,
    established: VecDeque<Arc<TcpConn>>,
}

enum State {
    Init { local_port: Option<u16> },
    Listening(Listening),
}

pub struct TcpSocket {
    io: Arc<Io>,
    wait_queue: WaitQueue,
    state: SpinLock<State>,
}

impl TcpSocket {
    pub fn new(io: Arc<Io>) -> Result<Arc<Self>, ErrorCode> {
        let wait_queue = WaitQueue::new()?;
        Ok(Arc::new(Self {
            io,
            wait_queue,
            state: SpinLock::new(State::Init { local_port: None }),
        }))
    }

    /// `bind(2)` implementation.
    pub fn do_bind(&self, port: u16) -> Result<(), Errno> {
        let mut state = self.state.lock();
        let State::Init { local_port } = &mut *state else {
            return Err(Errno::EINVAL);
        };

        if local_port.is_some() {
            // The socket is already bound to a port.
            return Err(Errno::EINVAL);
        }

        // Bind this port to the network to start receiving packets.
        self.io.bind_listener(port).map_err(Errno::from)?;
        *local_port = Some(port);
        Ok(())
    }

    /// `listen(2)` implementation.
    pub fn do_listen(&self, backlog: i32) -> Result<(), Errno> {
        if backlog < 0 {
            return Err(Errno::EINVAL);
        }

        let mut state = self.state.lock();
        match &mut *state {
            State::Init {
                local_port: Some(local_port),
            } => {
                *state = State::Listening(Listening {
                    local_port: *local_port,
                    backlog: backlog as usize,
                    inflights: Vec::new(),
                    established: VecDeque::new(),
                });
            }
            State::Listening(listening) => {
                // Already listening. Update the backlog.
                listening.backlog = backlog as usize;
            }
            _ => {
                // The socket is not bound to a port.
                return Err(Errno::EINVAL);
            }
        }

        Ok(())
    }

    /// `accept(2)` implementation (non-blocking).
    pub fn try_accept(&self) -> Result<Option<Arc<TcpConn>>, Errno> {
        match &mut *self.state.lock() {
            State::Listening(listening) => Ok(listening.established.pop_front()),
            _ => Err(Errno::EINVAL),
        }
    }

    /// Returns true if the `pkt` is targeted at this listener.
    pub fn matches(&self, pkt: &TcpPacketInfo) -> bool {
        match &*self.state.lock() {
            State::Listening(listening) => listening.local_port == pkt.local_port,
            _ => false,
        }
    }

    /// Returns true if the listener can accept a connection.
    pub fn can_accept(&self) -> bool {
        match &*self.state.lock() {
            State::Listening(listening) => listening.backlog > 0,
            _ => false,
        }
    }

    fn send_syn_ack(&self, handshake: &Handshake, local_port: u16) {
        let segment = Segment {
            seq: handshake.local_iss,
            ack: handshake.remote_rcv_nxt,
            window_size: TCP_BUFFER_SIZE as u16,
            flags: TcpFlags::SYN | TcpFlags::ACK,
            payload: &[],
        };

        if let Err(err) = self.io.send_segment(handshake.remote, local_port, segment) {
            trace!("failed to send SYN-ACK packet: {:?}", err);
        }
    }

    fn find_handshake(&self, listening: &Listening, remote: Endpoint) -> Option<usize> {
        for index in 0..listening.inflights.len() {
            if listening.inflights[index].remote == remote {
                return Some(index);
            }
        }

        None
    }

    fn reset_handshake(&self, pkt: &TcpPacketInfo) {
        let remote = Endpoint {
            ip: pkt.remote_ip,
            port: pkt.remote_port,
        };

        // Find the handshake with this peer.
        let mut state = self.state.lock();
        let State::Listening(listening) = &mut *state else {
            return;
        };

        let Some(index) = self.find_handshake(listening, remote) else {
            return;
        };

        let handshake = listening.inflights[index];
        if pkt.seq != handshake.remote_rcv_nxt {
            // Unexpected sequence number. Ignore this RST.
            return;
        }

        // Remove the handshake.
        listening.inflights.remove(index);
    }

    fn start_handshake(&self, pkt: &TcpPacketInfo) {
        let remote = Endpoint {
            ip: pkt.remote_ip,
            port: pkt.remote_port,
        };

        let mut state = self.state.lock();
        let State::Listening(listening) = &mut *state else {
            // The socket is not listening.
            return;
        };

        // Check if the handshake with the peer is already in progress.
        let local_port = listening.local_port;
        if let Some(index) = self.find_handshake(listening, remote) {
            let handshake = listening.inflights[index];
            drop(state);

            // Send a SYN-ACK packet again.
            self.send_syn_ack(&handshake, local_port);
            return;
        }

        // Do we have room for a new connection?
        if listening.inflights.len() + listening.established.len() >= listening.backlog {
            // No room for a new connection.
            return;
        }

        // Start a new handshake.
        // TODO: SYN cookie support
        let handshake = Handshake {
            remote,
            local_iss: INITIAL_SEND_SEQ,
            remote_rcv_nxt: pkt.seq.wrapping_add(1),
            remote_rcv_wnd: pkt.window_size,
        };
        listening.inflights.push(handshake);
        drop(state);
        self.send_syn_ack(&handshake, local_port);
    }

    // Receives an ACK, which is typically ACKing our SYN-ACK packet.
    fn finish_handshake(&self, pkt: &TcpPacketInfo, payload: &[u8], listener_io: &ListenerIo<'_>) {
        let remote = Endpoint {
            ip: pkt.remote_ip,
            port: pkt.remote_port,
        };

        // Find the handshake with this peer.
        let (handshake, local_port) = {
            let mut state = self.state.lock();
            let State::Listening(listening) = &mut *state else {
                // The socket is not listening.
                return;
            };

            let Some(index) = self.find_handshake(listening, remote) else {
                // No handshake with this peer.
                return;
            };

            let expected_ack = listening.inflights[index].local_iss.wrapping_add(1);
            if pkt.ack != expected_ack {
                // Unexpected ACK.
                return;
            }

            if pkt.seq != listening.inflights[index].remote_rcv_nxt {
                // Unexpected sequence number.
                return;
            }

            (listening.inflights.remove(index), listening.local_port)
        };

        // The connection has been established.
        let conn = match TcpConn::new_established(
            self.io.clone(),
            handshake.remote,
            local_port,
            handshake.local_iss,
            handshake.remote_rcv_nxt,
            handshake.remote_rcv_wnd,
        ) {
            Ok(conn) => conn,
            Err(error) => {
                trace!(
                    "failed to create connection: {:?}, dropping this connection",
                    error
                );
                return;
            }
        };

        if let Err(err) = listener_io.add_connection(conn.clone(), pkt) {
            trace!("failed to add connection: {:?}", err);
            return;
        }

        // Handle this packet in the new conn object.
        conn.handle_rx(pkt, payload);
        if conn.is_closed() {
            return;
        }

        // Add the connection to accept later.
        let mut state = self.state.lock();
        let State::Listening(listening) = &mut *state else {
            return;
        };
        listening.established.push_back(conn);
        drop(state);

        if let Err(error) = self.wait_queue.notify_all() {
            trace!("failed to notify listener waiters: {:?}", error);
        }
    }

    pub fn handle_rx(&self, pkt: &TcpPacketInfo, payload: &[u8], listener_io: ListenerIo<'_>) {
        let flags = TcpFlags::from_u8(pkt.flags);
        if flags.contains(TcpFlags::RST) {
            self.reset_handshake(pkt);
            return;
        }

        if flags.contains(TcpFlags::SYN) {
            self.start_handshake(pkt);
            return;
        }

        if flags.contains(TcpFlags::ACK) {
            self.finish_handshake(pkt, payload, &listener_io);
        }
    }
}

impl Drop for TcpSocket {
    fn drop(&mut self) {
        let mut state = self.state.lock();
        let local_port = match &mut *state {
            State::Init { local_port } => *local_port,
            State::Listening(listening) => {
                // Close all established (but not yet accepted) connections.
                for conn in listening.established.drain(..) {
                    conn.do_close();
                }

                Some(listening.local_port)
            }
        };

        // Unbind the listener from the port.
        drop(state);
        if let Some(port) = local_port {
            self.io.unbind_listener(port);
        }
    }
}

impl FileLike for TcpSocket {
    fn bind(&self, addr: SockAddr) -> Result<(), Errno> {
        match addr {
            SockAddr::Inet { ip, port } => {
                let _ = ip; // TODO: support binding to specific IP address
                self.do_bind(port)
            }
        }
    }

    fn listen(&self, backlog: c_int) -> Result<(), Errno> {
        self.do_listen(backlog)
    }

    fn setsockopt(
        &self,
        level: c_int,
        optname: c_int,
        _optval: *const u8,
        _optlen: usize,
    ) -> Result<(), Errno> {
        match (level, optname) {
            (SOL_SOCKET, SO_REUSEADDR) => {
                // TODO: Implement SO_REUSEADDR
                Ok(())
            }
            _ => Err(Errno::ENOTSUP),
        }
    }

    fn accept(&self, nonblocking: bool, sleep: Sleep<'_>) -> Result<Arc<dyn FileLike>, Errno> {
        let sleep_guard = sleep.guard(&self.wait_queue)?;
        loop {
            if let Some(conn) = self.try_accept()? {
                return Ok(conn);
            }

            if nonblocking {
                return Err(Errno::EAGAIN);
            }

            if sleep_guard.is_interrupted() {
                return Err(Errno::EINTR);
            }

            sleep_guard.wait()?;
        }
    }

    fn poll(&self) -> Result<c_short, Errno> {
        match &*self.state.lock() {
            State::Init { .. } => Ok(0),
            State::Listening(listening) if listening.established.is_empty() => Ok(0),
            State::Listening(_) => Ok(POLLIN),
        }
    }

    fn wait_queue(&self) -> Option<&WaitQueue> {
        Some(&self.wait_queue)
    }

    fn recvfrom(
        &self,
        _buf: &mut [u8],
        _flags: c_int,
        _nonblocking: bool,
        _sleep: Sleep<'_>,
    ) -> Result<(usize, SockAddr), Errno> {
        Err(Errno::ENOTCONN)
    }

    fn sendto(
        &self,
        _buf: &[u8],
        _dest: Option<SockAddr>,
        _flags: c_int,
        _nonblocking: bool,
        _sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        Err(Errno::ENOTCONN)
    }
}
