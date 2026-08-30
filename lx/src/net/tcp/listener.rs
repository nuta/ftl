use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec::Vec;

use ftl::info;
use ftl::syscall::poll_notify;
use ftl::syscall::poll_wait;
use ftl_types::handle::HandleId;
use ftl_types::net::NetRxInfo;
use ftl_utils::spinlock::SpinLock;

use super::buffer::TCP_BUFFER_CAPACITY;
use super::buffer::TcpBuffer;
use super::conn::TcpConnection;
use super::packet::Endpoint;
use super::packet::Segment;
use super::packet::TcpFlags;
use crate::net::TcpIp;
use crate::types::errno::Errno;
use crate::vfs::FileLike;

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
    inflight: Vec<Handshake>,
    established: VecDeque<Arc<TcpConnection>>,
}

pub struct TcpListener {
    network: Weak<TcpIp>,
    poll: HandleId,
    mutable: SpinLock<Mutable>,
}

impl TcpListener {
    pub fn new(network: Weak<TcpIp>, poll: HandleId) -> Arc<Self> {
        let mutable = Mutable {
            local_port: None,
            backlog: 0,
            inflight: Vec::new(),
            established: VecDeque::new(),
        };
        Arc::new(Self {
            network,
            poll,
            mutable: SpinLock::new(mutable),
        })
    }

    pub fn bind(&self, port: u16) -> Result<(), Errno> {
        if port != 80 {
            return Err(Errno::EINVAL);
        }

        let mut mutable = self.mutable.lock();
        if mutable.local_port.is_some() {
            return Err(Errno::EINVAL);
        }
        mutable.local_port = Some(port);
        Ok(())
    }

    pub fn listen(&self, backlog: i32) -> Result<(), Errno> {
        if backlog < 0 {
            return Err(Errno::EINVAL);
        }

        let mut mutable = self.mutable.lock();
        let Some(port) = mutable.local_port else {
            return Err(Errno::EINVAL);
        };
        mutable.backlog = backlog.max(1) as usize;
        drop(mutable);
        info!("tcpip: listening on {}", port);
        Ok(())
    }

    pub fn accepts(&self, port: u16) -> bool {
        let mutable = self.mutable.lock();
        mutable.backlog > 0 && mutable.local_port == Some(port)
    }

    fn network(&self) -> Option<Arc<TcpIp>> {
        self.network.upgrade()
    }

    fn send_syn_ack(&self, handshake: &Handshake, local_port: u16) {
        let Some(network) = self.network() else {
            return;
        };
        let segment = Segment {
            seq: handshake.local_iss,
            ack: handshake.remote_rcv_nxt,
            window_size: TCP_BUFFER_CAPACITY as u16,
            flags: TcpFlags::SYN | TcpFlags::ACK,
            payload: &[],
        };
        let _ = network.send_segment(handshake.remote, local_port, segment);
    }

    fn find_handshake(mutable: &Mutable, remote: Endpoint) -> Option<usize> {
        for index in 0..mutable.inflight.len() {
            if mutable.inflight[index].remote == remote {
                return Some(index);
            }
        }
        None
    }

    fn start_handshake(&self, info: &NetRxInfo) {
        let remote = Endpoint {
            ip: info.remote_ip,
            port: info.remote_port,
        };

        let mut mutable = self.mutable.lock();
        let Some(local_port) = mutable.local_port else {
            return;
        };
        if let Some(index) = Self::find_handshake(&mutable, remote) {
            let handshake = mutable.inflight[index];
            drop(mutable);
            self.send_syn_ack(&handshake, local_port);
            return;
        }
        if mutable.inflight.len() + mutable.established.len() >= mutable.backlog {
            return;
        }

        let handshake = Handshake {
            remote,
            local_iss: INITIAL_SEND_SEQ,
            remote_rcv_nxt: info.seq.wrapping_add(1),
            remote_rcv_wnd: info.window_size,
        };
        mutable.inflight.push(handshake);
        drop(mutable);
        self.send_syn_ack(&handshake, local_port);
    }

    fn finish_handshake(&self, info: &NetRxInfo, payload: &[u8]) {
        let remote = Endpoint {
            ip: info.remote_ip,
            port: info.remote_port,
        };

        let (handshake, local_port) = {
            let mut mutable = self.mutable.lock();
            let Some(index) = Self::find_handshake(&mutable, remote) else {
                return;
            };
            let expected_ack = mutable.inflight[index].local_iss.wrapping_add(1);
            if info.ack != expected_ack {
                return;
            }
            if info.seq != mutable.inflight[index].remote_rcv_nxt {
                return;
            }
            let Some(local_port) = mutable.local_port else {
                return;
            };
            (mutable.inflight.remove(index), local_port)
        };

        let Some(network) = self.network() else {
            return;
        };
        let connection = TcpConnection::new(
            Arc::downgrade(&network),
            self.poll,
            handshake.remote,
            local_port,
            handshake.local_iss,
            handshake.remote_rcv_nxt,
            handshake.remote_rcv_wnd,
            TcpBuffer::new(),
        );
        network.add_connection(connection.clone());
        connection.handle_packet(info, payload);
        if connection.is_closed() {
            return;
        }

        self.mutable.lock().established.push_back(connection);
        let _ = poll_notify(self.poll);
    }

    pub fn handle_packet(&self, info: &NetRxInfo, payload: &[u8]) {
        let flags = TcpFlags::from_u8(info.flags);
        if flags.contains(TcpFlags::RST) {
            return;
        }
        if flags.contains(TcpFlags::SYN) {
            self.start_handshake(info);
            return;
        }
        if flags.contains(TcpFlags::ACK) {
            self.finish_handshake(info, payload);
        }
    }

    pub fn accept(&self) -> Result<Arc<TcpConnection>, Errno> {
        loop {
            let connection = self.mutable.lock().established.pop_front();
            if let Some(connection) = connection {
                return Ok(connection);
            }
            poll_wait(self.poll)?;
        }
    }
}

impl FileLike for TcpListener {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
