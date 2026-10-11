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
use crate::types::errno::Errno;
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

struct Mutable {
    backlog: usize,
    inflights: Vec<Handshake>,
    established: VecDeque<Arc<TcpConn>>,
}

impl Mutable {
    fn find_handshake(&self, remote: Endpoint) -> Option<usize> {
        for index in 0..self.inflights.len() {
            if self.inflights[index].remote == remote {
                return Some(index);
            }
        }

        None
    }
}

pub struct TcpListener {
    io: Arc<Io>,
    wait_queue: Arc<WaitQueue>,
    local_port: u16,
    mutable: SpinLock<Mutable>,
}

impl TcpListener {
    pub fn new(
        io: Arc<Io>,
        wait_queue: Arc<WaitQueue>,
        local_port: u16,
        backlog: usize,
    ) -> Result<Arc<Self>, Errno> {
        // Bind the listening port.
        match io.bind_listener(local_port) {
            Ok(()) => {}
            Err(ErrorCode::AlreadyExists) => return Err(Errno::EADDRINUSE),
            Err(error) => return Err(error.into()),
        }

        Ok(Arc::new(Self {
            io,
            wait_queue,
            local_port,
            mutable: SpinLock::new(Mutable {
                backlog,
                inflights: Vec::new(),
                established: VecDeque::new(),
            }),
        }))
    }

    fn notify(&self) {
        if let Err(error) = self.wait_queue.notify_all() {
            trace!("failed to notify listener waiters: {:?}", error);
        }
    }

    pub fn set_backlog(&self, backlog: usize) {
        self.mutable.lock().backlog = backlog;
    }

    /// Returns true if the `pkt` is targeted at this listener.
    pub fn matches(&self, pkt: &TcpPacketInfo) -> bool {
        self.local_port == pkt.local_port
    }

    /// Returns true if the listener can accept a connection.
    pub fn can_accept(&self) -> bool {
        self.mutable.lock().backlog > 0
    }

    pub fn is_readable(&self) -> bool {
        !self.mutable.lock().established.is_empty()
    }

    /// `accept(2)` implementation (non-blocking).
    pub fn try_accept(&self) -> Option<Arc<TcpConn>> {
        self.mutable.lock().established.pop_front()
    }

    pub fn do_accept(&self, nonblocking: bool, sleep: Sleep<'_>) -> Result<Arc<TcpConn>, Errno> {
        let sleep_guard = sleep.guard(&self.wait_queue)?;
        loop {
            if let Some(conn) = self.try_accept() {
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

    fn send_syn_ack(&self, handshake: &Handshake) {
        let segment = Segment {
            seq: handshake.local_iss,
            ack: handshake.remote_rcv_nxt,
            window_size: TCP_BUFFER_SIZE as u16,
            flags: TcpFlags::SYN | TcpFlags::ACK,
            payload: &[],
        };

        if let Err(err) = self
            .io
            .send_segment(handshake.remote, self.local_port, segment)
        {
            trace!("failed to send SYN-ACK packet: {:?}", err);
        }
    }

    fn reset_handshake(&self, pkt: &TcpPacketInfo) {
        let remote = Endpoint {
            ip: pkt.remote_ip,
            port: pkt.remote_port,
        };

        // Find the handshake with this peer.
        let mut mutable = self.mutable.lock();
        let Some(index) = mutable.find_handshake(remote) else {
            return;
        };

        let handshake = mutable.inflights[index];
        if pkt.seq != handshake.remote_rcv_nxt {
            // Unexpected sequence number. Ignore this RST.
            return;
        }

        // Remove the handshake.
        mutable.inflights.remove(index);
    }

    fn start_handshake(&self, pkt: &TcpPacketInfo) {
        let remote = Endpoint {
            ip: pkt.remote_ip,
            port: pkt.remote_port,
        };

        let mut mutable = self.mutable.lock();

        // Check if the handshake with the peer is already in progress.
        if let Some(index) = mutable.find_handshake(remote) {
            let handshake = mutable.inflights[index];
            drop(mutable);

            // Send a SYN-ACK packet again.
            self.send_syn_ack(&handshake);
            return;
        }

        // Do we have room for a new connection?
        if mutable.inflights.len() + mutable.established.len() >= mutable.backlog {
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
        mutable.inflights.push(handshake);
        drop(mutable);
        self.send_syn_ack(&handshake);
    }

    // Receives an ACK, which is typically ACKing our SYN-ACK packet.
    fn finish_handshake(&self, pkt: &TcpPacketInfo, payload: &[u8]) {
        let remote = Endpoint {
            ip: pkt.remote_ip,
            port: pkt.remote_port,
        };

        // Find the handshake with this peer.
        let handshake = {
            let mut mutable = self.mutable.lock();
            let Some(index) = mutable.find_handshake(remote) else {
                // No handshake with this peer.
                return;
            };

            let expected_ack = mutable.inflights[index].local_iss.wrapping_add(1);
            if pkt.ack != expected_ack {
                // Unexpected ACK.
                return;
            }

            if pkt.seq != mutable.inflights[index].remote_rcv_nxt {
                // Unexpected sequence number.
                return;
            }

            mutable.inflights.remove(index)
        };

        // The connection has been established.
        let conn = match TcpConn::new_established(
            self.io.clone(),
            handshake.remote,
            self.local_port,
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

        if let Err(err) = self.io.add_connection(conn.clone(), pkt) {
            trace!("failed to add connection: {:?}", err);
            return;
        }

        // Handle this packet in the new conn object.
        conn.handle_rx(pkt, payload);
        if conn.is_closed() {
            return;
        }

        // Add the connection to accept later.
        self.mutable.lock().established.push_back(conn);
        self.notify();
    }

    pub fn handle_rx(&self, pkt: &TcpPacketInfo, payload: &[u8]) {
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
            self.finish_handshake(pkt, payload);
        }
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        // Close all connections in the backlog.
        for conn in self.mutable.lock().established.drain(..) {
            conn.do_close();
        }

        // Unbind the listening port.
        self.io.unbind_listener(self.local_port);
    }
}
