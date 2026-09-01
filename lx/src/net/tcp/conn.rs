use alloc::sync::Arc;
use core::cmp::min;

use ftl::trace;
use ftl_types::error::ErrorCode;
use ftl_utils::spinlock::SpinLock;

use super::buffer::TcpBuffer;
use super::packet::Endpoint;
use super::packet::Segment;
use super::packet::TcpFlags;
use super::packet::TcpPacketInfo;
use crate::net::tcpip::Io;
use crate::net::tcpip::RecvGuard;
use crate::types::c_short;
use crate::types::errno::Errno;
use crate::types::sys::poll::POLLIN;
use crate::types::sys::poll::POLLOUT;
use crate::types::sys::socket::SockAddr;
use crate::vfs::FileLike;
use crate::wait_queue::WaitQueue;

// TODO: Should we make this configurable?
const MAX_SEGMENT_DATA_LEN: usize = 1460;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Established,
    CloseWait,
    LastAck,
    FinWait1,
    FinWait2,
    Closing,
    Closed,
}

struct Mutable {
    state: State,
    closing: bool,
    snd_una: u32,
    snd_nxt: u32,
    snd_wnd: u16,
    rcv_nxt: u32,
    tx_buffer: TcpBuffer,
    rx_buffer: TcpBuffer,
    eof: bool,
}

pub struct TcpConn {
    io: Arc<Io>,
    wait_queue: WaitQueue,
    remote: Endpoint,
    local_port: u16,
    mutable: SpinLock<Mutable>,
}

impl TcpConn {
    pub fn new_established(
        io: Arc<Io>,
        remote: Endpoint,
        local_port: u16,
        local_iss: u32,
        remote_rcv_nxt: u32,
        remote_rcv_wnd: u16,
        rx_buffer: TcpBuffer,
    ) -> Result<Arc<Self>, ErrorCode> {
        let wait_queue = WaitQueue::new()?;
        let snd_nxt = local_iss.wrapping_add(1);
        let mutable = Mutable {
            state: State::Established,
            closing: false,
            snd_una: snd_nxt,
            snd_nxt,
            snd_wnd: remote_rcv_wnd,
            rcv_nxt: remote_rcv_nxt,
            tx_buffer: TcpBuffer::new(),
            rx_buffer,
            eof: false,
        };

        Ok(Arc::new(Self {
            io,
            wait_queue,
            remote,
            local_port,
            mutable: SpinLock::new(mutable),
        }))
    }

    fn notify(&self) {
        if let Err(error) = self.wait_queue.notify_all() {
            trace!("failed to notify connection waiters: {:?}", error);
        }
    }

    pub fn is_closed(&self) -> bool {
        self.mutable.lock().state == State::Closed
    }

    /// Send a segment.
    fn send(&self, segment: Segment) -> Result<(), Errno> {
        self.io
            .send_segment(self.remote, self.local_port, segment)
            .map_err(Errno::from)
    }

    /// Sends an ACK.
    fn send_ack(&self, mutable: &Mutable) {
        let window_size = mutable.rx_buffer.writable_len() as u16;
        let segment = Segment {
            seq: mutable.snd_nxt,
            ack: mutable.rcv_nxt,
            window_size,
            flags: TcpFlags::ACK,
            payload: &[],
        };

        if let Err(error) = self.send(segment) {
            trace!("failed to send ACK: {:?}", error);
        }
    }

    /// Sends a FIN.
    fn send_fin(&self, mutable: &mut Mutable) {
        let window_size = mutable.rx_buffer.writable_len() as u16;
        let segment = Segment {
            seq: mutable.snd_nxt,
            ack: mutable.rcv_nxt,
            window_size,
            flags: TcpFlags::FIN | TcpFlags::ACK,
            payload: &[],
        };

        if let Err(error) = self.send(segment) {
            trace!("failed to send FIN: {:?}", error);
            return;
        }

        // Update our state.
        mutable.snd_nxt = mutable.snd_nxt.wrapping_add(1);
        mutable.state = match mutable.state {
            State::Established => State::FinWait1,
            State::CloseWait => State::LastAck,
            state => state,
        };
    }

    fn send_data(&self, mutable: &mut Mutable) {
        let inflight_len = mutable.snd_nxt.wrapping_sub(mutable.snd_una) as usize;
        let window_len = mutable.snd_wnd as usize;
        let sendable_len = window_len.saturating_sub(inflight_len);
        let sendable_len = min(sendable_len, MAX_SEGMENT_DATA_LEN);
        let Some(payload) = mutable.tx_buffer.peek(inflight_len, sendable_len) else {
            return;
        };

        let window_size = mutable.rx_buffer.writable_len() as u16;
        let payload_len = payload.len();
        let segment = Segment {
            seq: mutable.snd_nxt,
            ack: mutable.rcv_nxt,
            window_size,
            flags: TcpFlags::ACK | TcpFlags::PSH,
            payload,
        };

        match self.send(segment) {
            Ok(_) => {
                mutable.snd_nxt = mutable.snd_nxt.wrapping_add(payload_len as u32);
            }
            Err(e) => trace!("failed to send data: {:?}", e),
        }
    }

    fn flush(&self, mutable: &mut Mutable) {
        if mutable.state != State::Established && mutable.state != State::CloseWait {
            // Notthing to send.
            return;
        }

        if mutable.closing && mutable.tx_buffer.is_empty() {
            // We've sent all data, and the peer has acknowledged it. It's time
            // to send a FIN.
            self.send_fin(mutable);
            return;
        }

        self.send_data(mutable);
    }

    /// Handles an ACK.
    fn handle_ack(&self, mutable: &mut Mutable, ack: u32) -> Option<usize> {
        let acked_len = ack.wrapping_sub(mutable.snd_una) as usize;
        let inflight_len = mutable.snd_nxt.wrapping_sub(mutable.snd_una) as usize;
        if acked_len > inflight_len {
            return None;
        }

        if acked_len > 0 {
            mutable.snd_una = ack;
            mutable.tx_buffer.consume(acked_len);
        }

        if mutable.snd_una != mutable.snd_nxt {
            return Some(acked_len);
        }

        mutable.state = match mutable.state {
            State::FinWait1 => State::FinWait2,
            State::Closing | State::LastAck => State::Closed,
            state => state,
        };

        Some(acked_len)
    }

    /// Handles a FIN.
    fn handle_fin(&self, mutable: &mut Mutable, pkt: &TcpPacketInfo) {
        let fin_seq = pkt.seq.wrapping_add(pkt.payload_len as u32);
        if fin_seq != mutable.rcv_nxt {
            return;
        }

        mutable.rcv_nxt = mutable.rcv_nxt.wrapping_add(1);
        mutable.eof = true;
        mutable.state = match mutable.state {
            State::Established => State::CloseWait,
            State::FinWait1 => State::Closing,
            State::FinWait2 => State::Closed,
            state => state,
        };
    }

    pub fn handle_rx(&self, pkt: &TcpPacketInfo, recv_guard: RecvGuard<'_>) {
        let flags = TcpFlags::from_u8(pkt.flags);
        let mut mutable = self.mutable.lock();

        // Handle out-of-order segments.
        if pkt.seq != mutable.rcv_nxt {
            // Send an ACK to tell the peer to resend the segment we want.
            self.send_ack(&mutable);
            return;
        }

        mutable.snd_wnd = pkt.window_size;

        // Handle RST.
        if flags.contains(TcpFlags::RST) {
            mutable.state = State::Closed;
            mutable.eof = true;
            drop(mutable);
            self.notify();
            return;
        }

        // Handle ACK.
        let acked_len = if flags.contains(TcpFlags::ACK) {
            match self.handle_ack(&mut mutable, pkt.ack) {
                Some(len) => len,
                None => return,
            }
        } else {
            0
        };

        // Receive the TCP payload into the RX buffer.
        let received_len = pkt.payload_len as usize;
        let written_len = mutable.rx_buffer.write_with(received_len, |payload| {
            match recv_guard.recv(payload) {
                Ok(_) => payload.len(),
                Err(e) => {
                    trace!("failed to receive network payload: {:?}", e);
                    0
                }
            }
        });

        // Writes to the RX buffer may fail on OOM.
        let written_len = match written_len {
            Ok(len) => len,
            Err(e) => {
                trace!("failed to write to RX buffer: {:?}", e);
                0
            }
        };

        if written_len > 0 {
            // Update the sequence number, and send an ACK to the peer.
            mutable.rcv_nxt = mutable.rcv_nxt.wrapping_add(written_len as u32);
        }

        // Handle FIN.
        let received_fin = flags.contains(TcpFlags::FIN);
        if flags.contains(TcpFlags::FIN) {
            self.handle_fin(&mut mutable, pkt);
        }

        let prev_snd_nxt = mutable.snd_nxt;
        self.flush(&mut mutable);
        if mutable.snd_nxt == prev_snd_nxt && (received_len > 0 || received_fin) {
            // Flush did not send any data. Send an ACK to the peer.
            self.send_ack(&mutable);
        }
        drop(mutable);

        if acked_len > 0 || received_len > 0 || received_fin {
            let _ = self.notify();
        }
    }

    /// Initiates a connection close.
    pub fn do_close(&self) {
        let mut mutable = self.mutable.lock();
        mutable.closing = true;
        self.flush(&mut mutable);
    }
}

impl FileLike for TcpConn {
    fn read(&self, buf: &mut [u8], _offset: usize, nonblocking: bool) -> Result<usize, Errno> {
        if buf.is_empty() {
            return Ok(0);
        }

        let wq = self.wait_queue.subscribe();
        loop {
            let mut mutable = self.mutable.lock();
            if !mutable.rx_buffer.is_empty() {
                let len = mutable.rx_buffer.read(buf);

                // We've consumed some data. Tell the peer the new window size.
                self.send_ack(&mutable);

                return Ok(len);
            }

            if mutable.eof || mutable.state == State::Closed {
                // RX buffer is empty, and the connection is closed.
                return Ok(0);
            }

            if nonblocking {
                return Err(Errno::EAGAIN);
            }

            drop(mutable);
            wq.wait()?;
        }
    }

    fn write(&self, buf: &[u8], _offset: usize, nonblocking: bool) -> Result<usize, Errno> {
        if buf.is_empty() {
            return Ok(0);
        }

        let wq = self.wait_queue.subscribe();
        loop {
            let mut mutable = self.mutable.lock();
            if mutable.state != State::Established && mutable.state != State::CloseWait {
                // The connection is not in a writable state.
                // TODO: return an appropriate errno
                return Err(Errno::EINVAL);
            }

            let written_len = mutable.tx_buffer.write(buf);
            if written_len > 0 {
                self.flush(&mut mutable);
                return Ok(written_len);
            }

            if nonblocking {
                return Err(Errno::EAGAIN);
            }

            drop(mutable);
            wq.wait()?;
        }
    }

    fn peer_addr(&self) -> Result<SockAddr, Errno> {
        Ok(SockAddr::Inet {
            ip: self.remote.ip,
            port: self.remote.port,
        })
    }

    fn close(&self) {
        self.do_close();
    }

    fn poll(&self) -> Result<c_short, Errno> {
        let mut status = 0;
        let mutable = self.mutable.lock();

        // This is readable if read() would return 0 anyway (EOF/closed).
        if !mutable.rx_buffer.is_empty() || mutable.eof || mutable.state == State::Closed {
            status |= POLLIN;
        }

        if (mutable.state == State::Established || mutable.state == State::CloseWait)
            && mutable.tx_buffer.writable_len() > 0
        {
            status |= POLLOUT;
        }

        Ok(status)
    }

    fn wait_queue(&self) -> Option<&WaitQueue> {
        Some(&self.wait_queue)
    }
}
