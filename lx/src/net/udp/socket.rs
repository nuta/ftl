use alloc::sync::Arc;
use core::cmp::min;
use core::mem::size_of;

use ftl::time::MonoTime;
use ftl::time::MonoTimeExt;
use ftl_types::error::ErrorCode;
use ftl_types::time::Duration;
use ftl_utils::spinlock::SpinLock;

use super::rx_queue::RxQueue;
use crate::net::ipv4::Endpoint;
use crate::net::tcpip::BindGuard;
use crate::net::tcpip::TcpIp;
use crate::types::c_int;
use crate::types::c_short;
use crate::types::errno::Errno;
use crate::types::sys::poll::POLLIN;
use crate::types::sys::poll::POLLOUT;
use crate::types::sys::socket::SO_RCVTIMEO;
use crate::types::sys::socket::SO_REUSEADDR;
use crate::types::sys::socket::SOL_SOCKET;
use crate::types::sys::socket::SockAddr;
use crate::types::sys::time::TimeVal;
use crate::vfs::FileLike;
use crate::wait_queue::Sleep;
use crate::wait_queue::WaitQueue;

const MAX_PAYLOAD_LEN: usize = 1500 - 20 - 8;

struct Mutable {
    bind_guard: Option<Arc<BindGuard>>,
    recv_timeout: Option<Duration>,
}

pub struct UdpSocket {
    tcpip: Arc<TcpIp>,
    wait_queue: Arc<WaitQueue>,
    rx_queue: Arc<RxQueue>,
    mutable: SpinLock<Mutable>,
}

impl UdpSocket {
    pub fn new(tcpip: Arc<TcpIp>) -> Result<Arc<Self>, ErrorCode> {
        let wait_queue = Arc::new(WaitQueue::new()?);
        Ok(Arc::new(Self {
            tcpip,
            rx_queue: Arc::new(RxQueue::new(wait_queue.clone())),
            wait_queue,
            mutable: SpinLock::new(Mutable {
                bind_guard: None,
                recv_timeout: None,
            }),
        }))
    }

    fn do_bind(&self, port: u16) -> Result<(), Errno> {
        let mut mutable = self.mutable.lock();
        if mutable.bind_guard.is_some() {
            return Err(Errno::EINVAL);
        }

        let bind_guard = self.tcpip.bind_udp(Some(port), self.rx_queue.clone())?;
        mutable.bind_guard = Some(bind_guard);
        Ok(())
    }

    /// Ensures the socket has a local port.
    ///
    /// UDP sockets don't have to call bind(2) before sending packets, and the
    /// kernel assigns an ephemeral port.
    fn ensure_bound(&self) -> Result<u16, Errno> {
        let mut mutable = self.mutable.lock();
        if let Some(bind_guard) = &mutable.bind_guard {
            return Ok(bind_guard.port());
        }

        let bind_guard = self.tcpip.bind_udp(None, self.rx_queue.clone())?;
        let port = bind_guard.port();
        mutable.bind_guard = Some(bind_guard);
        Ok(port)
    }

    fn send(&self, buf: &[u8], dest: Option<SockAddr>) -> Result<usize, Errno> {
        let remote = match (dest, self.rx_queue.peer()) {
            // The peer address is explicitly provided.
            (Some(SockAddr::Inet { ip, port }), _) => Endpoint { ip, port },
            // The peer address is implicitly set by connect(2).
            (None, Some(peer)) => peer,
            // We don't where to send the packet.
            (None, None) => return Err(Errno::EDESTADDRREQ),
        };

        if remote.ip == 0 || remote.port == 0 {
            return Err(Errno::EINVAL);
        }

        if buf.len() > MAX_PAYLOAD_LEN {
            return Err(Errno::EMSGSIZE);
        }

        let local_port = self.ensure_bound()?;
        self.tcpip.send_udp(remote, local_port, &buf.to_vec())?;
        Ok(buf.len())
    }

    fn get_recv_deadline(&self, now: MonoTime) -> Option<MonoTime> {
        let mutable = self.mutable.lock();
        let Some(timeout) = mutable.recv_timeout else {
            return None;
        };

        Some(now + timeout)
    }

    fn recv(
        &self,
        buf: &mut [u8],
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<(usize, Endpoint), Errno> {
        let deadline = self.get_recv_deadline(MonoTime::now());
        let sleep_guard = sleep.guard(&self.wait_queue)?;
        loop {
            if let Some(datagram) = self.rx_queue.pop() {
                // Received a datagram.
                let len = min(buf.len(), datagram.payload.len());
                buf[..len].copy_from_slice(&datagram.payload[..len]);
                return Ok((len, datagram.src));
            }

            if nonblocking {
                return Err(Errno::EAGAIN);
            }

            if sleep_guard.is_interrupted() {
                return Err(Errno::EINTR);
            }

            match deadline {
                Some(deadline) => {
                    if sleep_guard.wait_until(deadline)? {
                        // The deadline was reached.
                        return Err(Errno::EAGAIN);
                    }
                }
                None => sleep_guard.wait()?,
            }
        }
    }
}

impl FileLike for UdpSocket {
    fn bind(&self, addr: SockAddr) -> Result<(), Errno> {
        match addr {
            SockAddr::Inet { ip, port } => {
                let _ = ip;
                self.do_bind(port)
            }
        }
    }

    /// Sets the remote (peer) address for the socket.
    fn connect(&self, addr: SockAddr, _nonblocking: bool, _sleep: Sleep<'_>) -> Result<(), Errno> {
        match addr {
            SockAddr::Inet { ip, port } => {
                if ip == 0 || port == 0 {
                    return Err(Errno::EINVAL);
                }

                // Assign a local port if not already bound.
                self.ensure_bound()?;

                self.rx_queue.connect(Endpoint { ip, port });
                Ok(())
            }
        }
    }

    fn setsockopt(
        &self,
        level: c_int,
        optname: c_int,
        optval: *const u8,
        optlen: usize,
    ) -> Result<(), Errno> {
        match (level, optname) {
            (SOL_SOCKET, SO_REUSEADDR) => Ok(()),
            (SOL_SOCKET, SO_RCVTIMEO) => {
                if optval.is_null() || optlen < size_of::<TimeVal>() {
                    return Err(Errno::EINVAL);
                }

                let timeval = unsafe { optval.cast::<TimeVal>().read_unaligned() };
                let nanos = timeval.to_nanos().ok_or(Errno::EINVAL)?;

                let mut mutable = self.mutable.lock();
                mutable.recv_timeout = match nanos {
                    0 => None,
                    _ => Some(Duration::from_nanos(nanos)),
                };
                Ok(())
            }
            _ => Err(Errno::ENOTSUP),
        }
    }

    fn poll(&self) -> Result<c_short, Errno> {
        let mut status = POLLOUT;
        if !self.rx_queue.is_empty() {
            status |= POLLIN;
        }

        Ok(status)
    }

    fn wait_queue(&self) -> Option<&WaitQueue> {
        Some(&self.wait_queue)
    }

    fn peer_addr(&self) -> Result<SockAddr, Errno> {
        let peer = self.rx_queue.peer().ok_or(Errno::ENOTCONN)?;
        Ok(SockAddr::Inet {
            ip: peer.ip,
            port: peer.port,
        })
    }

    fn close(&self) {
        if let Some(bind_guard) = self.mutable.lock().bind_guard.take() {
            self.tcpip.unbind_udp(bind_guard.port());
        }
    }

    fn recvfrom(
        &self,
        buf: &mut [u8],
        _flags: c_int,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<(usize, SockAddr), Errno> {
        let (n, src) = self.recv(buf, nonblocking, sleep)?;
        let addr = SockAddr::Inet {
            ip: src.ip,
            port: src.port,
        };
        Ok((n, addr))
    }

    fn sendto(
        &self,
        buf: &[u8],
        dest: Option<SockAddr>,
        _flags: c_int,
        _nonblocking: bool,
        _sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        self.send(buf, dest)
    }

    fn read(
        &self,
        buf: &mut [u8],
        _offset: usize,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        let (n, _) = self.recv(buf, nonblocking, sleep)?;
        Ok(n)
    }

    fn write(
        &self,
        buf: &[u8],
        _offset: usize,
        _nonblocking: bool,
        _sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        self.send(buf, None)
    }
}
