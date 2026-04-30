use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;

use crate::TcpIp;
use crate::io::Io;
use crate::ip::IpAddr;
use crate::packet::Packet;
use crate::socket::AnySocket;
use crate::socket::Endpoint;
use crate::socket::SocketMap;
use crate::tcp::Accept;
use crate::tcp::RingBuffer;
use crate::tcp::TcpConn;
use crate::tcp::connection::DEFAULT_RCV_WND;
use crate::tcp::header::TcpFlags;
use crate::tcp::header::TcpHeader;
use crate::tcp::rx::RxHeader;
use crate::tcp::tx::transmit_segment;
use crate::transport::Port;

struct Handshake {
    remote: Endpoint,
    local_ip: IpAddr,
    local_iss: u32,
    remote_rcv_nxt: u32,
    remote_rcv_wnd: u16,
    rx_buffer: RingBuffer,
}

struct PendingAccept<I: Io> {
    request: I::TcpAccept,
    conn: Arc<TcpConn<I>>,
}

struct Mutable<I: Io> {
    /// Handshakes that we have sent SYN+ACK, and are waiting for an ACK.
    inflight_handshakes: Vec<Handshake>,
    /// Handshakes that we have received an ACK for. Established connections.
    finished_handshakes: VecDeque<Handshake>,
    pending_accepts: VecDeque<PendingAccept<I>>,
}

#[derive(Debug)]
pub enum AcceptError {}

pub struct TcpListener<I: Io> {
    local_port: Port,
    mutable: spin::Mutex<Mutable<I>>,
}

impl<I: Io> TcpListener<I> {
    pub(crate) fn new(local_port: Port) -> Self {
        Self {
            local_port,
            mutable: spin::Mutex::new(Mutable {
                inflight_handshakes: Vec::new(),
                finished_handshakes: VecDeque::new(),
                pending_accepts: VecDeque::new(),
            }),
        }
    }

    pub fn accept(
        &self,
        tcpip: &mut TcpIp<I>,
        request: I::TcpAccept,
    ) -> Result<Arc<TcpConn<I>>, AcceptError> {
        let conn = Arc::new(TcpConn::new_listen(self.local_port));
        let pending_accept = PendingAccept {
            request,
            conn: conn.clone(),
        };

        let mut mutable = self.mutable.lock();
        if let Some(h) = mutable.finished_handshakes.pop_front() {
            self.accept_handshake(tcpip, pending_accept, h);
        } else {
            mutable.pending_accepts.push_back(pending_accept);
        }
        Ok(conn)
    }

    fn start_handshake(self: &Arc<Self>, tcpip: &mut TcpIp<I>, rx: RxHeader) {
        let our_iss = 1234; // TODO: generate a random ISS.

        // TODO: Backlog limit: ensure mutable.handshakes.len() + mutable.finished_handshakes.len() <= MAX_BACKLOG.

        let remote = Endpoint {
            addr: rx.remote_ip,
            port: rx.src_port,
        };

        let handshake = Handshake {
            remote,
            local_ip: rx.local_ip,
            local_iss: our_iss,
            remote_rcv_nxt: rx.seq.wrapping_add(1),
            remote_rcv_wnd: rx.window_size,
            rx_buffer: RingBuffer::new(),
        };

        let header = TcpHeader {
            src_port: self.local_port.into(),
            dst_port: handshake.remote.port.into(),
            seq: handshake.local_iss.into(),
            ack: handshake.remote_rcv_nxt.into(),
            window_size: DEFAULT_RCV_WND.into(),
            flags: TcpFlags::SYN | TcpFlags::ACK,
            header_len: 0.into(),
            checksum: 0.into(),
            urgent_pointer: 0.into(),
        };

        let mut mutable = self.mutable.lock();
        mutable.inflight_handshakes.push(handshake);
        drop(mutable);

        if let Err(err) = transmit_segment::<I>(tcpip, header, rx.remote_ip, &[]) {
            warn!("TCP: failed to reply to SYN: {:?}", err);
        }
    }

    fn finish_handshake(
        self: &Arc<Self>,
        tcpip: &mut TcpIp<I>,
        rx: RxHeader,
        payload: &mut Packet,
    ) {
        let mut mutable = self.mutable.lock();

        // Find the matching handshake.
        let Some((index, _)) = mutable
            .inflight_handshakes
            .iter()
            .enumerate()
            .find(|(_, h)| rx.remote_ip == h.remote.addr && rx.src_port == h.remote.port)
        else {
            debug!("TCP: no SYN found for {}:{}", rx.remote_ip, rx.src_port);
            return;
        };

        let mut h = mutable.inflight_handshakes.remove(index);
        if !payload.is_empty() {
            if rx.seq != h.remote_rcv_nxt {
                trace!(
                    "TCP: out-of-order data on final ACK: seq={}, rcv_nxt={}",
                    rx.seq, h.remote_rcv_nxt
                );
            } else {
                // Data on the third leg consumes receive sequence space before TcpConn sees more packets.
                let written_len = h.rx_buffer.write_bytes(payload.slice());
                h.remote_rcv_nxt = h.remote_rcv_nxt.wrapping_add(written_len as u32);

                let header = TcpHeader {
                    src_port: self.local_port.into(),
                    dst_port: h.remote.port.into(),
                    seq: h.local_iss.wrapping_add(1).into(),
                    ack: h.remote_rcv_nxt.into(),
                    window_size: (h.rx_buffer.writeable_len() as u16).into(),
                    flags: TcpFlags::ACK,
                    header_len: 0.into(),
                    checksum: 0.into(),
                    urgent_pointer: 0.into(),
                };

                // ACK to the payload.
                if let Err(err) = transmit_segment::<I>(tcpip, header, h.remote.addr, &[]) {
                    warn!("TCP: failed to ACK final ACK payload: {:?}", err);
                }
            }
        }

        if let Some(pending_accept) = mutable.pending_accepts.pop_front() {
            // Register the connection immediately.
            self.accept_handshake(tcpip, pending_accept, h);
        } else {
            // Queue this established connection for later completion.
            mutable.finished_handshakes.push_back(h);
        }
    }

    fn accept_handshake(
        &self,
        tcpip: &mut TcpIp<I>,
        pending_accept: PendingAccept<I>,
        h: Handshake,
    ) {
        let PendingAccept { request, conn } = pending_accept;

        conn.open_passively(
            h.remote,
            h.local_iss,
            h.remote_rcv_nxt,
            h.remote_rcv_wnd,
            h.rx_buffer,
        );

        let local = Endpoint {
            addr: h.local_ip,
            port: self.local_port,
        };
        match tcpip.sockets.tcp_establish(h.remote, local, conn.clone()) {
            Ok(()) => {
                request.complete(Ok(()));
            }
            Err(err) => {
                warn!("TCP: failed to insert active connection: {:?}", err);
                request.complete(Ok(())); // TODO: return an error
            }
        }
    }

    pub(super) fn handle_rx(
        self: &Arc<Self>,
        tcpip: &mut TcpIp<I>,
        rx: RxHeader,
        payload: &mut Packet,
    ) {
        match rx.flags {
            TcpFlags::SYN => {
                self.start_handshake(tcpip, rx);
            }
            _ if rx.flags.contains(TcpFlags::ACK) => {
                self.finish_handshake(tcpip, rx, payload);
            }
            _ => {
                debug!("TCP: unexpected flags: {:?}", rx.flags);
                // TODO: Send an RST packet.
            }
        }
    }
}

impl<I: Io> AnySocket for TcpListener<I> {}

impl<I: Io> fmt::Debug for TcpListener<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpListener").finish()
    }
}
