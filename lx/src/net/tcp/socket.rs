use alloc::sync::Arc;

use ftl_types::error::ErrorCode;
use ftl_types::nic::PROTO_IPV4;
use ftl_types::nic::PROTO_TCP;
use ftl_utils::spinlock::SpinLock;

use super::conn::TcpConn;
use super::listener::TcpListener;
use super::packet::Endpoint;
use crate::net::tcpip::BindGuard;
use crate::net::tcpip::TcpIp;
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

enum State {
    Init { bind_guard: Option<Arc<BindGuard>> },
    Listener(Arc<TcpListener>),
    Conn(Arc<TcpConn>),
}

pub struct TcpSocket {
    tcpip: Arc<TcpIp>,
    wait_queue: Arc<WaitQueue>,
    state: SpinLock<State>,
}

impl TcpSocket {
    pub fn new(tcpip: Arc<TcpIp>) -> Result<Arc<Self>, ErrorCode> {
        let wait_queue = Arc::new(WaitQueue::new()?);
        Ok(Arc::new(Self {
            tcpip,
            wait_queue,
            state: SpinLock::new(State::Init { bind_guard: None }),
        }))
    }

    fn as_listener(&self) -> Result<Arc<TcpListener>, Errno> {
        match &*self.state.lock() {
            State::Listener(listener) => Ok(listener.clone()),
            _ => Err(Errno::EINVAL),
        }
    }

    fn as_conn(&self) -> Result<Arc<TcpConn>, Errno> {
        match &*self.state.lock() {
            State::Conn(conn) => Ok(conn.clone()),
            _ => Err(Errno::ENOTCONN),
        }
    }

    /// `bind(2)` implementation.
    pub fn do_bind(&self, port: u16) -> Result<(), Errno> {
        let mut state = self.state.lock();
        let State::Init { bind_guard } = &mut *state else {
            return Err(Errno::EINVAL);
        };

        if bind_guard.is_some() {
            // The socket is already bound to a port.
            return Err(Errno::EINVAL);
        }

        if port == 0 {
            return Ok(());
        }

        if self.tcpip.has_tcp_listener(port) {
            // FIXME: Reserve the port, not just checking if it's in use.
            return Err(Errno::EADDRINUSE);
        }

        *bind_guard = Some(self.tcpip.bind_port(PROTO_IPV4 | PROTO_TCP, port)?);
        Ok(())
    }

    /// `listen(2)` implementation.
    pub fn do_listen(&self, backlog: i32) -> Result<(), Errno> {
        if backlog < 0 {
            return Err(Errno::EINVAL);
        }

        let mut state = self.state.lock();
        match &mut *state {
            State::Init { bind_guard } => {
                let bind_guard = match bind_guard {
                    Some(bind_guard) => bind_guard.clone(),
                    None => self.tcpip.alloc_tcp_port()?,
                };
                let listener = TcpListener::new(
                    self.tcpip.clone(),
                    self.wait_queue.clone(),
                    bind_guard,
                    backlog as usize,
                )?;
                *state = State::Listener(listener);
            }
            State::Listener(listener) => {
                // Already listening. Update the backlog.
                listener.set_backlog(backlog as usize);
            }
            _ => {
                // The socket is connected.
                return Err(Errno::EINVAL);
            }
        }

        Ok(())
    }

    /// `connect(2)` implementation.
    pub fn do_connect(
        &self,
        remote: Endpoint,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<(), Errno> {
        let conn = {
            let mut state = self.state.lock();
            match &*state {
                State::Init { bind_guard } => {
                    // Start a handshake.
                    let wq = self.wait_queue.clone();
                    let conn = self.tcpip.connect_tcp(remote, bind_guard.clone(), wq)?;
                    *state = State::Conn(conn.clone());
                    conn
                }
                State::Conn(conn) if conn.is_connecting() => {
                    // Already connecting. Just wait for the handshake to
                    // complete.
                    conn.clone()
                }
                _ => {
                    return Err(Errno::EISCONN);
                }
            }
        };

        conn.wait_for_handshake(nonblocking, sleep)
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

    fn connect(&self, addr: SockAddr, nonblocking: bool, sleep: Sleep<'_>) -> Result<(), Errno> {
        match addr {
            SockAddr::Inet { ip, port } => {
                self.do_connect(Endpoint { ip, port }, nonblocking, sleep)
            }
        }
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
        let conn = self.as_listener()?.do_accept(nonblocking, sleep)?;
        Ok(conn)
    }

    fn poll(&self) -> Result<c_short, Errno> {
        match &*self.state.lock() {
            State::Init { .. } => Ok(0),
            State::Listener(listener) if listener.is_readable() => Ok(POLLIN),
            State::Listener(_) => Ok(0),
            State::Conn(conn) => FileLike::poll(conn.as_ref()),
        }
    }

    fn wait_queue(&self) -> Option<&WaitQueue> {
        Some(&self.wait_queue)
    }

    fn peer_addr(&self) -> Result<SockAddr, Errno> {
        self.as_conn()?.peer_addr()
    }

    fn close(&self) {
        match &*self.state.lock() {
            State::Init { .. } => {}
            State::Listener(listener) => listener.close(),
            State::Conn(conn) => conn.do_close(),
        }
    }

    fn shutdown(&self, how: c_int) -> Result<(), Errno> {
        self.as_conn()?.do_shutdown(how)
    }

    fn recvfrom(
        &self,
        buf: &mut [u8],
        flags: c_int,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<(usize, SockAddr), Errno> {
        self.as_conn()?.recvfrom(buf, flags, nonblocking, sleep)
    }

    fn sendto(
        &self,
        buf: &[u8],
        dest: Option<SockAddr>,
        flags: c_int,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        self.as_conn()?.sendto(buf, dest, flags, nonblocking, sleep)
    }

    fn read(
        &self,
        buf: &mut [u8],
        offset: usize,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        self.as_conn()?.read(buf, offset, nonblocking, sleep)
    }

    fn write(
        &self,
        buf: &[u8],
        offset: usize,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        self.as_conn()?.write(buf, offset, nonblocking, sleep)
    }
}
