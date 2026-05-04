use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::fmt;
use core::time::Duration;

use hashbrown::HashMap;

use crate::TcpIp;
use crate::io::Instant;
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

const SYN_RECEIVED_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_LISTEN_BACKLOG: usize = 256;

struct Handshake<I: Io> {
    remote: Endpoint,
    local_ip: IpAddr,
    local_iss: u32,
    remote_rcv_nxt: u32,
    remote_rcv_wnd: u16,
    rx_buffer: TcpBuffer,
    expires_at: I::Instant,
}

struct PendingAccept<I: Io> {
    request: I::TcpAccept,
    conn: Arc<TcpConn<I>>,
}

struct Mutable<I: Io> {
    /// Handshakes that we have sent SYN+ACK, and are waiting for an ACK.
    inflight_handshakes: HashMap<Endpoint, Handshake<I>>,
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

    pub(crate) fn handle_timeout(&self, now: &I::Instant) -> Option<I::Instant> {
        let mut next: Option<I::Instant> = None;
        let mut expired = 0;
        let mut mutable = self.mutable.lock();
        mutable.inflight_handshakes.retain(|_, handshake| {
            if now.is_before(&handshake.expires_at) {
                if !matches!(next, Some(current) if current.is_before(&handshake.expires_at)) {
                    next = Some(handshake.expires_at);
                }

                true
            } else {
                expired += 1;
                false
            }
        });
        drop(mutable);

        if expired > 0 {
            trace!("TCP: expired {} inflight handshakes", expired);
        }

        next
    }

    fn start_handshake(self: &Arc<Self>, tcpip: &mut TcpIp<I>, rx: RxHeader) {
        let our_iss = 1234; // TODO: generate a random ISS.

        let remote = Endpoint {
            addr: rx.remote_ip,
            port: rx.src_port,
        };
        let now = tcpip.io_mut().now();
        let expires_at = now.checked_add(SYN_RECEIVED_TIMEOUT).unwrap();

        let handshake = Handshake {
            remote,
            local_ip: rx.local_ip,
            local_iss: our_iss,
            remote_rcv_nxt: rx.seq.wrapping_add(1),
            remote_rcv_wnd: rx.window_size,
            rx_buffer: TcpBuffer::new(),
            expires_at,
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
        if mutable.inflight_handshakes.contains_key(&remote) {
            mutable.inflight_handshakes.insert(remote, handshake);
        } else {
            let backlog_len = mutable.inflight_handshakes.len() + mutable.finished_handshakes.len();
            if backlog_len >= MAX_LISTEN_BACKLOG {
                debug!(
                    "TCP: listen backlog full; dropping handshake from {}",
                    remote.addr
                );
                return;
            }

            if let Err(err) = mutable
                .inflight_handshakes
                .reserve_and_insert(remote, handshake)
            {
                warn!("TCP: failed to insert handshake: {:?}", err);
                // TODO: RST the connection.
                return;
            }
        }
        drop(mutable);
        tcpip.io_mut().set_timer(expires_at);

        if let Err(err) = transmit_segment::<I>(tcpip, header, rx.remote_ip, &[]) {
            warn!("TCP: failed to reply to SYN: {:?}", err);
        }
    }

    fn reset_handshake(&self, rx: RxHeader) {
        let remote = Endpoint {
            addr: rx.remote_ip,
            port: rx.src_port,
        };

        let mut mutable = self.mutable.lock();
        if mutable.inflight_handshakes.remove(&remote).is_some() {
            trace!(
                "TCP: reset inflight handshake from {}:{}",
                remote.addr, remote.port
            );
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
            _ if rx.flags.contains(TcpFlags::RST) => {
                self.reset_handshake(rx);
            }
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

#[cfg(test)]
mod tests {
    use alloc::sync::Arc;
    use core::time::Duration;

    use super::*;
    use crate::ethernet::MacAddr;
    use crate::interface::Device;
    use crate::ip::Ipv4Addr;
    use crate::packet::Packet;
    use crate::tcp::Accept;
    use crate::tcp::Error;
    use crate::tcp::Read;
    use crate::tcp::Write;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct TestInstant(u64);

    impl Instant for TestInstant {
        fn checked_add(&self, duration: Duration) -> Option<Self> {
            Some(Self(
                self.0.checked_add(duration.as_nanos().try_into().ok()?)?,
            ))
        }

        fn now(&self) -> Self {
            *self
        }

        fn is_before(&self, other: &Self) -> bool {
            self.0 < other.0
        }

        fn elapsed_since(&self, other: &Self) -> Duration {
            Duration::from_nanos(self.0 - other.0)
        }
    }

    struct TestDevice;

    impl Device for TestDevice {
        fn mac_addr(&self) -> &MacAddr {
            static MAC: MacAddr = MacAddr::new([0; 6]);
            &MAC
        }

        fn transmit(&mut self, _pkt: &mut Packet) {}
    }

    struct TestRead;
    impl Read for TestRead {
        fn complete(self, _rx_buffer: &mut TcpBuffer) {}
    }

    struct TestWrite;
    impl Write for TestWrite {
        fn complete(self, _tx_buffer: &mut TcpBuffer) {}
    }

    struct TestAccept;
    impl Accept for TestAccept {
        fn complete(self, _result: Result<(), Error>) {}
    }

    struct TestIo;

    impl Io for TestIo {
        type Device = TestDevice;
        type TcpWrite = TestWrite;
        type TcpRead = TestRead;
        type TcpAccept = TestAccept;
        type Instant = TestInstant;

        fn now(&self) -> Self::Instant {
            TestInstant(0)
        }

        fn set_timer(&mut self, _at: Self::Instant) {}
    }

    fn endpoint(port: u16) -> Endpoint {
        Endpoint {
            addr: IpAddr::V4(Ipv4Addr::new(192, 0, 2, (port & 0xff) as u8)),
            port: Port::new(port),
        }
    }

    fn handshake(expires_at: u64) -> Handshake<TestIo> {
        Handshake {
            remote: endpoint(1),
            local_ip: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 254)),
            local_iss: 0,
            remote_rcv_nxt: 0,
            remote_rcv_wnd: DEFAULT_RCV_WND,
            rx_buffer: TcpBuffer::new(),
            expires_at: TestInstant(expires_at),
        }
    }

    #[test]
    fn handle_timeout_keeps_earliest_inflight_handshake() {
        let listener = TcpListener::<TestIo>::new(Port::new(80));
        {
            let mut mutable = listener.mutable.lock();
            mutable
                .inflight_handshakes
                .insert(endpoint(10), handshake(10));
            mutable
                .inflight_handshakes
                .insert(endpoint(20), handshake(20));
        }

        assert_eq!(
            listener.handle_timeout(&TestInstant(5)),
            Some(TestInstant(10))
        );
        assert_eq!(listener.mutable.lock().inflight_handshakes.len(), 2);
    }

    #[test]
    fn handle_timeout_expires_inflight_handshakes() {
        let listener = TcpListener::<TestIo>::new(Port::new(80));
        {
            let mut mutable = listener.mutable.lock();
            mutable
                .inflight_handshakes
                .insert(endpoint(10), handshake(10));
            mutable
                .inflight_handshakes
                .insert(endpoint(20), handshake(20));
        }

        assert_eq!(
            listener.handle_timeout(&TestInstant(10)),
            Some(TestInstant(20))
        );
        assert!(
            !listener
                .mutable
                .lock()
                .inflight_handshakes
                .contains_key(&endpoint(10))
        );
        assert_eq!(listener.mutable.lock().inflight_handshakes.len(), 1);

        assert_eq!(listener.handle_timeout(&TestInstant(20)), None);
        assert!(listener.mutable.lock().inflight_handshakes.is_empty());
    }

    #[test]
    fn rst_removes_inflight_handshake() {
        let listener = Arc::new(TcpListener::<TestIo>::new(Port::new(80)));
        let remote = endpoint(10);
        let mut handshake = handshake(10);
        handshake.remote = remote;
        {
            let mut mutable = listener.mutable.lock();
            mutable.inflight_handshakes.insert(remote, handshake);
        }

        let mut tcpip = TcpIp::<TestIo>::new(TestIo);
        let mut payload = Packet::new(0, 0).unwrap();
        listener.handle_rx(
            &mut tcpip,
            RxHeader {
                remote_ip: remote.addr,
                local_ip: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 254)),
                src_port: remote.port,
                dst_port: Port::new(80),
                flags: TcpFlags::RST,
                seq: 0,
                ack: 0,
                window_size: 0,
            },
            &mut payload,
        );

        assert!(
            !listener
                .mutable
                .lock()
                .inflight_handshakes
                .contains_key(&remote)
        );
    }
}
