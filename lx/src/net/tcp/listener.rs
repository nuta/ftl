use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;

use ftl::trace;
use ftl_types::error::ErrorCode;
use ftl_utils::spinlock::SpinLock;

use super::buffer::TCP_BUFFER_SIZE;
use super::buffer::TcpBuffer;
use super::conn::TcpConn;
use super::packet::Endpoint;
use super::packet::Segment;
use super::packet::TcpFlags;
use super::packet::TcpPacketInfo;
use crate::net::tcpip::Io;
use crate::net::tcpip::ListenerIo;
use crate::net::tcpip::RecvGuard;
use crate::types::c_int;
use crate::types::c_short;
use crate::types::errno::Errno;
use crate::types::sys::poll::POLLIN;
use crate::types::sys::socket::SockAddr;
use crate::vfs::FileLike;
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
    local_port: Option<u16>,
    backlog: usize,
    inflights: Vec<Handshake>,
    established: VecDeque<Arc<TcpConn>>,
}

pub struct TcpListener {
    io: Arc<Io>,
    wait_queue: WaitQueue,
    mutable: SpinLock<Mutable>,
}

impl TcpListener {
    pub fn new(io: Arc<Io>) -> Result<Arc<Self>, ErrorCode> {
        let wait_queue = WaitQueue::new()?;
        let mutable = Mutable {
            local_port: None,
            backlog: 0,
            inflights: Vec::new(),
            established: VecDeque::new(),
        };
        Ok(Arc::new(Self {
            io,
            wait_queue,
            mutable: SpinLock::new(mutable),
        }))
    }

    /// `bind(2)` implementation.
    pub fn do_bind(&self, port: u16) -> Result<(), Errno> {
        let mut mutable = self.mutable.lock();
        if mutable.local_port.is_some() {
            // The listener is already bound to a port.
            return Err(Errno::EINVAL);
        }

        // Bind this port to the network to start receiving packets.
        self.io.bind_listener(port).map_err(Errno::from)?;
        mutable.local_port = Some(port);
        Ok(())
    }

    /// `listen(2)` implementation.
    pub fn do_listen(&self, backlog: i32) -> Result<(), Errno> {
        if backlog < 0 {
            return Err(Errno::EINVAL);
        }

        let mut mutable = self.mutable.lock();
        if mutable.local_port.is_none() {
            // The listener is not bound to a port.
            return Err(Errno::EINVAL);
        }

        mutable.backlog = backlog as usize;
        Ok(())
    }

    /// `accept(2)` implementation (non-blocking).
    pub fn try_accept(&self) -> Option<Arc<TcpConn>> {
        self.mutable.lock().established.pop_front()
    }

    /// Returns true if the `pkt` is targeted at this listener.
    pub fn matches(&self, pkt: &TcpPacketInfo) -> bool {
        let mutable = self.mutable.lock();
        mutable.local_port == Some(pkt.local_port)
    }

    /// Returns true if the listener can accept a connection.
    pub fn can_accept(&self) -> bool {
        let mutable = self.mutable.lock();
        mutable.backlog > 0
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

    fn find_handshake(&self, mutable: &Mutable, remote: Endpoint) -> Option<usize> {
        for index in 0..mutable.inflights.len() {
            if mutable.inflights[index].remote == remote {
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
        let mut mutable = self.mutable.lock();
        let Some(index) = self.find_handshake(&mutable, remote) else {
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
        let Some(local_port) = mutable.local_port else {
            // The listener is not bound to a port.
            return;
        };

        // Check if the handshake with the peer is already in progress.
        if let Some(index) = self.find_handshake(&mutable, remote) {
            let handshake = mutable.inflights[index];
            drop(mutable);

            // Send a SYN-ACK packet again.
            self.send_syn_ack(&handshake, local_port);
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
        self.send_syn_ack(&handshake, local_port);
    }

    // Receives an ACK, which is typically ACKing our SYN-ACK packet.
    fn finish_handshake(
        &self,
        pkt: &TcpPacketInfo,
        recv_guard: RecvGuard<'_>,
        listener_io: &ListenerIo<'_>,
    ) {
        let remote = Endpoint {
            ip: pkt.remote_ip,
            port: pkt.remote_port,
        };

        // Find the handshake with this peer.
        let (handshake, local_port) = {
            let mut mutable = self.mutable.lock();
            let Some(local_port) = mutable.local_port else {
                // The listener is not bound to a port.
                return;
            };

            let Some(index) = self.find_handshake(&mutable, remote) else {
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

            (mutable.inflights.remove(index), local_port)
        };

        // The connection has been established.
        let conn = match TcpConn::new_established(
            self.io.clone(),
            handshake.remote,
            local_port,
            handshake.local_iss,
            handshake.remote_rcv_nxt,
            handshake.remote_rcv_wnd,
            TcpBuffer::new(),
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
        conn.handle_rx(pkt, recv_guard);
        if conn.is_closed() {
            return;
        }

        // Add the connection to accept later.
        self.mutable.lock().established.push_back(conn);

        if let Err(error) = self.wait_queue.notify_all() {
            trace!("failed to notify listener waiters: {:?}", error);
        }
    }

    pub fn handle_rx(
        &self,
        pkt: &TcpPacketInfo,
        recv_guard: RecvGuard<'_>,
        listener_io: ListenerIo<'_>,
    ) {
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
            self.finish_handshake(pkt, recv_guard, &listener_io);
        }
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        let mut mutable = self.mutable.lock();

        // Close all established (but not yet accepted) connections.
        for conn in mutable.established.drain(..) {
            conn.do_close();
        }

        // Unbind the listener from the port.
        let local_port = mutable.local_port;
        drop(mutable);
        if let Some(port) = local_port {
            self.io.unbind_listener(port);
        }
    }
}

impl FileLike for TcpListener {
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

    fn accept(&self) -> Result<Arc<dyn FileLike>, Errno> {
        let wq = self.wait_queue.subscribe();
        loop {
            if let Some(conn) = self.try_accept() {
                return Ok(conn);
            }

            wq.wait()?;
        }
    }

    fn poll(&self) -> Result<c_short, Errno> {
        let mut status = 0;

        let mutable = self.mutable.lock();
        if !mutable.established.is_empty() {
            status |= POLLIN;
        }

        Ok(status)
    }

    fn wait_queue(&self) -> Option<&WaitQueue> {
        Some(&self.wait_queue)
    }
}
