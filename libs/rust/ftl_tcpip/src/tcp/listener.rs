use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::fmt;

use hashbrown::HashMap;

use crate::TcpIp;
use crate::io::Io;
use crate::ip::IpAddr;
use crate::packet::Packet;
use crate::socket::Endpoint;
use crate::tcp::Accept;
use crate::tcp::TcpBuffer;
use crate::tcp::connection::DEFAULT_RCV_WND;
use crate::tcp::connection::TcpConn;
use crate::tcp::header::TcpFlags;
use crate::tcp::header::TcpHeader;
use crate::tcp::rx::RxHeader;
use crate::tcp::tx::transmit_segment;
use crate::transport::Port;
use crate::utils::HashMapExt;

struct Handshake {
    remote: Endpoint,
    local_ip: IpAddr,
    local_iss: u32,
    remote_rcv_nxt: u32,
    remote_rcv_wnd: u16,
    rx_buffer: TcpBuffer,
}

struct PendingAccept<I: Io> {
    request: I::TcpAccept,
    conn: Arc<TcpConn<I>>,
}

struct Mutable<I: Io> {
    /// Handshakes that we have sent SYN+ACK, and are waiting for an ACK.
    inflight_handshakes: HashMap<Endpoint, Handshake>,
    /// Handshakes that we have received an ACK for. Established connections.
    finished_handshakes: VecDeque<Arc<TcpConn<I>>>,
    pending_accepts: VecDeque<PendingAccept<I>>,
}

pub struct TcpListener<I: Io> {
    local_port: Port,
    mutable: spin::Mutex<Mutable<I>>,
}

impl<I: Io> TcpListener<I> {
    pub(crate) fn new(local_port: Port) -> Self {
        Self {
            local_port,
            mutable: spin::Mutex::new(Mutable {
                inflight_handshakes: HashMap::new(),
                finished_handshakes: VecDeque::new(),
                pending_accepts: VecDeque::new(),
            }),
        }
    }

    pub fn accept(&self, request: I::TcpAccept) -> Arc<TcpConn<I>> {
        let mut mutable = self.mutable.lock();
        if let Some(conn) = mutable.finished_handshakes.pop_front() {
            drop(mutable);
            request.complete(Ok(()));
            conn
        } else {
            let conn = Arc::new(TcpConn::new_listen(self.local_port));
            let pending_accept = PendingAccept {
                request,
                conn: conn.clone(),
            };
            mutable.pending_accepts.push_back(pending_accept);
            conn
        }
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
            rx_buffer: TcpBuffer::new(),
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
        if let Err(err) = mutable
            .inflight_handshakes
            .reserve_and_insert(remote, handshake)
        {
            warn!("TCP: failed to insert handshake: {:?}", err);
            // TODO: RST the connection.
            return;
        }
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
        // Find the matching handshake.
        let (mut h, pending_accept) = {
            let mut mutable = self.mutable.lock();

            let remote = Endpoint {
                addr: rx.remote_ip,
                port: rx.src_port,
            };
            let Some(h) = mutable.inflight_handshakes.remove(&remote) else {
                debug!("TCP: no SYN found for {}:{}", rx.remote_ip, rx.src_port);
                return;
            };

            let pending_accept = mutable.pending_accepts.pop_front();
            (h, pending_accept)
        };

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

        let local = Endpoint {
            addr: h.local_ip,
            port: self.local_port,
        };

        if let Some(PendingAccept { request, conn }) = pending_accept {
            conn.open_passively(
                h.remote,
                h.local_iss,
                h.remote_rcv_nxt,
                h.remote_rcv_wnd,
                h.rx_buffer,
            );

            if let Err(err) = tcpip
                .sockets_mut()
                .establish_tcp_conn(h.remote, local, conn)
            {
                warn!("TCP: failed to establish TCP connection: {:?}", err);
                request.complete(Ok(())); // TODO: return an error
                return;
            }

            request.complete(Ok(()));
        } else {
            // Create a new connection and keep it until the application calls tcp_accept.
            let conn = Arc::new(TcpConn::new_listen(self.local_port));
            conn.open_passively(
                h.remote,
                h.local_iss,
                h.remote_rcv_nxt,
                h.remote_rcv_wnd,
                h.rx_buffer,
            );

            if let Err(err) = tcpip
                .sockets_mut()
                .establish_tcp_conn(h.remote, local, conn.clone())
            {
                warn!("TCP: failed to establish TCP connection: {:?}", err);
                // TODO: RST the connection.
                return;
            }

            let mut mutable = self.mutable.lock();
            mutable.finished_handshakes.push_back(conn);
        }
    }

    pub(super) fn handle_rx(
        self: &Arc<Self>,
        tcpip: &mut TcpIp<I>,
        rx: RxHeader,
        payload: &mut Packet,
    ) {
        match rx.flags {
            _ if rx.flags.contains(TcpFlags::SYN) => {
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

impl<I: Io> fmt::Debug for TcpListener<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpListener").finish()
    }
}
