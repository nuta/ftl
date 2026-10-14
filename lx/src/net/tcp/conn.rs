use alloc::sync::Arc;
use core::cmp::min;

use ftl::time::MonoTime;
use ftl::time::MonoTimeExt;
use ftl::trace;
use ftl_types::error::ErrorCode;
use ftl_types::time::Duration;
use ftl_utils::spinlock::SpinLock;
use ftl_utils::spinlock::SpinLockGuard;

use super::buffer::TCP_BUFFER_SIZE;
use super::buffer::TcpBuffer;
use super::packet::Endpoint;
use super::packet::Segment;
use super::packet::TcpFlags;
use super::packet::TcpPacketInfo;
use crate::net::tcpip::Io;
use crate::types::c_int;
use crate::types::c_short;
use crate::types::errno::Errno;
use crate::types::sys::poll::POLLERR;
use crate::types::sys::poll::POLLIN;
use crate::types::sys::poll::POLLOUT;
use crate::types::sys::socket::SHUT_RD;
use crate::types::sys::socket::SHUT_RDWR;
use crate::types::sys::socket::SHUT_WR;
use crate::types::sys::socket::SockAddr;
use crate::vfs::FileLike;
use crate::wait_queue::Sleep;
use crate::wait_queue::WaitQueue;

// TODO: Should we make this configurable?
const MAX_SEGMENT_DATA_LEN: usize = 1460;
const CLOSING_TIMEOUT: Duration = Duration::from_secs(5);
const SYN_TIMEOUT_SECS: u64 = 1;
const MAX_SYN_RETRIES: u32 = 5;
const RTO_SECS: u64 = 1;
const MAX_RTO_RETRIES: u32 = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    SynSent,
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
    write_closed: bool,
    read_closed: bool,
    snd_una: u32,
    snd_nxt: u32,
    snd_wnd: u16,
    rcv_nxt: u32,
    tx_buffer: TcpBuffer,
    rx_buffer: TcpBuffer,
    eof: bool,
    closing_deadline: Option<MonoTime>,
    syn_deadline: Option<MonoTime>,
    syn_retries: u32,
    rto_deadline: Option<MonoTime>,
    rto_retries: u32,
    error: Option<Errno>,
}

pub struct TcpConn {
    io: Arc<Io>,
    wait_queue: Arc<WaitQueue>,
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
    ) -> Result<Arc<Self>, ErrorCode> {
        let wq = WaitQueue::new()?;
        let conn = Self::new_active(io, Arc::new(wq), remote, local_port, local_iss)?;
        let mut mutable = conn.mutable.lock();
        mutable.state = State::Established;
        mutable.snd_una = mutable.snd_nxt;
        mutable.snd_wnd = remote_rcv_wnd;
        mutable.rcv_nxt = remote_rcv_nxt;
        drop(mutable);
        Ok(conn)
    }

    pub fn new_active(
        io: Arc<Io>,
        wait_queue: Arc<WaitQueue>,
        remote: Endpoint,
        local_port: u16,
        local_iss: u32,
    ) -> Result<Arc<Self>, ErrorCode> {
        let mutable = Mutable {
            state: State::SynSent,
            write_closed: false,
            read_closed: false,
            snd_una: local_iss,
            snd_nxt: local_iss.wrapping_add(1),
            snd_wnd: 0,
            rcv_nxt: 0,
            tx_buffer: TcpBuffer::new()?,
            rx_buffer: TcpBuffer::new()?,
            eof: false,
            closing_deadline: None,
            syn_deadline: None,
            syn_retries: 0,
            rto_deadline: None,
            rto_retries: 0,
            error: None,
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

    pub fn is_connecting(&self) -> bool {
        self.mutable.lock().state == State::SynSent
    }

    pub fn poll(&self, now: MonoTime) {
        let mut mutable = self.mutable.lock();

        // Active open connections.
        if mutable.state == State::SynSent {
            let expired = match mutable.syn_deadline {
                Some(deadline) => now.duration_since(deadline).is_some(),
                None => false,
            };

            if !expired {
                // Too early to retransmit a SYN.
                return;
            }

            if mutable.syn_retries >= MAX_SYN_RETRIES {
                self.destroy(mutable, Errno::ETIMEDOUT);
                return;
            }

            // Retransmit a SYN.
            mutable.syn_retries += 1;
            if let Err(err) = self.send_syn(&mut mutable) {
                trace!("failed to retransmit SYN: {:?}", err);
            }

            return;
        }

        // Check retransmission timeout.
        if mutable.state != State::Closed
            && let Some(deadline) = mutable.rto_deadline
            && now.duration_since(deadline).is_some()
        {
            if mutable.rto_retries >= MAX_RTO_RETRIES {
                // Too many retransmissions. Give up and destroy the
                // connection.
                self.destroy(mutable, Errno::ETIMEDOUT);
                return;
            }

            // Retransmit the segment.
            mutable.rto_retries += 1;
            self.retransmit(&mut mutable);
        }

        let Some(deadline) = mutable.closing_deadline else {
            return;
        };

        if now.duration_since(deadline).is_none() {
            return;
        }

        // The deadline has passed. Mark the connection as closed.
        mutable.state = State::Closed;
        mutable.eof = true;
        drop(mutable);
        self.notify();
    }

    /// Send a segment.
    fn send(&self, segment: Segment) -> Result<(), Errno> {
        self.io
            .send_tcp_segment(self.remote, self.local_port, segment)
            .map_err(Errno::from)
    }

    /// Aborts a connection with an error.
    fn destroy(&self, mut mutable: SpinLockGuard<'_, Mutable>, error: Errno) {
        mutable.state = State::Closed;
        mutable.eof = true;
        mutable.error = Some(error);
        drop(mutable);
        self.notify();
    }

    /// Starts an active-open connection.
    ///
    /// This method does not return an error. It will retry sending the SYN
    /// later if it fails.
    pub fn start_active(&self) {
        let mut mutable = self.mutable.lock();
        if let Err(err) = self.send_syn(&mut mutable) {
            trace!("failed to send SYN: {:?}", err);
        }
    }

    /// Send a SYN packet, and arm a timeout for retransmission.
    fn send_syn(&self, mutable: &mut Mutable) -> Result<(), Errno> {
        let timeout = Duration::from_secs(SYN_TIMEOUT_SECS << mutable.syn_retries);
        mutable.syn_deadline = Some(MonoTime::now() + timeout);
        self.send(Segment {
            seq: mutable.snd_una,
            ack: 0,
            window_size: TCP_BUFFER_SIZE as u16,
            flags: TcpFlags::SYN,
            payload: &[],
        })
    }

    fn receive_syn_sent(&self, mut mutable: SpinLockGuard<'_, Mutable>, pkt: &TcpPacketInfo) {
        let flags = TcpFlags::from_u8(pkt.flags);
        if !flags.contains(TcpFlags::ACK) {
            // A reply to our SYN must set ACK.
            return;
        }

        if pkt.ack != mutable.snd_nxt {
            // The peer did not acknowledge our SYN. Send a RST if they didn't.
            if !flags.contains(TcpFlags::RST) {
                let segment = Segment {
                    seq: pkt.ack,
                    ack: 0,
                    window_size: 0,
                    flags: TcpFlags::RST,
                    payload: &[],
                };

                if let Err(err) = self.send(segment) {
                    trace!("failed to send RST: {:?}", err);
                }
            }

            return;
        }

        if flags.contains(TcpFlags::RST) {
            self.destroy(mutable, Errno::ECONNREFUSED);
            return;
        }

        if !flags.contains(TcpFlags::SYN) {
            return;
        }

        mutable.state = State::Established;
        mutable.snd_una = pkt.ack;
        mutable.snd_wnd = pkt.window_size;
        mutable.rcv_nxt = pkt.seq.wrapping_add(1);
        self.send_ack(&mutable);

        self.flush(&mut mutable);
        drop(mutable);
        self.notify();
    }

    /// Blocks until the connection completes (or fails) the handshake.
    pub fn wait_for_handshake(&self, nonblocking: bool, sleep: Sleep<'_>) -> Result<(), Errno> {
        let sleep_guard = sleep.guard(&self.wait_queue)?;
        loop {
            let mut mutable = self.mutable.lock();
            if let Some(error) = mutable.error.take() {
                return Err(error);
            }

            if mutable.state != State::SynSent {
                return Ok(());
            }

            if nonblocking {
                return Err(Errno::EINPROGRESS);
            }

            if sleep_guard.is_interrupted() {
                return Err(Errno::EINTR);
            }

            drop(mutable);
            sleep_guard.wait()?;
        }
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
            // Log but continue anyway. We'll resend a FIN later after the
            // retransmission timer expires.
            trace!("failed to send FIN: {:?}", error);
        }

        // Arm a retransmission timer if it's not set.
        if mutable.rto_deadline.is_none() {
            self.reset_rto(mutable);
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

        if let Err(e) = self.send(segment) {
            // Log but continue anyway. We'll resend the data later after the
            // retransmission timer expires.
            trace!("failed to send data: {:?}", e);
        }

        mutable.snd_nxt = mutable.snd_nxt.wrapping_add(payload_len as u32);

        // Arm a retransmission timer if it's not set.
        if mutable.rto_deadline.is_none() {
            self.reset_rto(mutable);
        }
    }

    /// Retransmits a segment (data or FIN).
    fn retransmit(&self, mutable: &mut Mutable) {
        self.reset_rto(mutable);

        let inflight_len = mutable.snd_nxt.wrapping_sub(mutable.snd_una) as usize;
        let len = min(inflight_len, MAX_SEGMENT_DATA_LEN);
        let (flags, payload) = match mutable.tx_buffer.peek(0, len) {
            // Retransmit data.
            Some(payload) => (TcpFlags::ACK | TcpFlags::PSH, payload),
            // Retransmit a FIN.
            None => (TcpFlags::FIN | TcpFlags::ACK, &[][..]),
        };

        let segment = Segment {
            seq: mutable.snd_una,
            ack: mutable.rcv_nxt,
            window_size: mutable.rx_buffer.writable_len() as u16,
            flags,
            payload,
        };

        if let Err(error) = self.send(segment) {
            trace!("failed to retransmit: {:?}", error);
        }
    }

    /// Resets the retransmission timer.
    fn reset_rto(&self, mutable: &mut Mutable) {
        let timeout = Duration::from_secs(RTO_SECS << mutable.rto_retries);
        mutable.rto_deadline = Some(MonoTime::now() + timeout);
    }

    fn flush(&self, mutable: &mut Mutable) {
        if mutable.state != State::Established && mutable.state != State::CloseWait {
            // Notthing to send.
            return;
        }

        if mutable.write_closed && mutable.tx_buffer.is_empty() {
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
            // Delayed ACK, where ack < snd_una, causes suspiciously very large
            // acked lengths. Ignore the acknowledgement, but continue processing
            // the segment since it may contain new data or FIN.
            if acked_len >= 1 << 31 {
                return Some(0);
            }

            return None;
        }

        if acked_len > 0 {
            mutable.snd_una = ack;
            mutable.tx_buffer.consume(acked_len);

            // The peer has acknowledged. This means the peer is still around,
            // so reset the retransmission timer.
            mutable.rto_retries = 0;
            self.reset_rto(mutable);
        }

        if mutable.snd_una != mutable.snd_nxt {
            // The peer has acknowledged part of the inflight data.
            return Some(acked_len);
        }

        // No more inflight data. No data to retransmit.
        mutable.rto_deadline = None;

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
            State::FinWait1 => {
                mutable.closing_deadline = Some(MonoTime::now() + CLOSING_TIMEOUT);
                State::Closing
            }
            State::FinWait2 => State::Closed,
            state => state,
        };
    }

    pub fn handle_rx(&self, pkt: &TcpPacketInfo, payload: &[u8]) {
        let flags = TcpFlags::from_u8(pkt.flags);
        let mut mutable = self.mutable.lock();
        if mutable.state == State::SynSent {
            self.receive_syn_sent(mutable, pkt);
            return;
        }

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

        let received_len = payload.len();
        let written_len = if mutable.read_closed {
            // Process won't read any more data. Discard the payload, but
            // acknowledge it.
            Ok(received_len)
        } else {
            // Receive the TCP payload into the RX buffer.
            mutable.rx_buffer.write_with(received_len, |buf| {
                if buf.len() != received_len {
                    return 0;
                }

                buf.copy_from_slice(payload);
                received_len
            })
        };

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
        mutable.write_closed = true;

        if mutable.state == State::SynSent {
            // The connection is not established yet, and it is safe to close
            // this socket immediately.
            mutable.state = State::Closed;
        }

        self.flush(&mut mutable);
    }

    pub fn do_shutdown(&self, how: c_int) -> Result<(), Errno> {
        // Determine what to do.
        let (shutdown_read, shutdown_write) = match how {
            SHUT_RD => (true, false),
            SHUT_WR => (false, true),
            SHUT_RDWR => (true, true),
            _ => {
                return Err(Errno::EINVAL);
            }
        };

        let mut mutable = self.mutable.lock();
        if mutable.state == State::Closed {
            return Err(Errno::ENOTCONN);
        }

        // The process won't read any more data.
        if shutdown_read {
            mutable.read_closed = true;
            mutable.rx_buffer.clear();
        }

        // The process won't write any more data.
        if shutdown_write {
            mutable.write_closed = true;
            self.flush(&mut mutable);
        }

        drop(mutable);
        self.notify();
        Ok(())
    }

    fn recv(&self, buf: &mut [u8], nonblocking: bool, sleep: Sleep<'_>) -> Result<usize, Errno> {
        if buf.is_empty() {
            return Ok(0);
        }

        let sleep_guard = sleep.guard(&self.wait_queue)?;
        loop {
            let mut mutable = self.mutable.lock();
            if let Some(error) = mutable.error.take() {
                return Err(error);
            }

            if mutable.read_closed {
                return Ok(0);
            }

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

            if sleep_guard.is_interrupted() {
                return Err(Errno::EINTR);
            }

            drop(mutable);
            sleep_guard.wait()?;
        }
    }
}

impl FileLike for TcpConn {
    fn recvfrom(
        &self,
        buf: &mut [u8],
        _flags: c_int,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<(usize, SockAddr), Errno> {
        // TODO: Implement flags
        let n = self.recv(buf, nonblocking, sleep)?;
        let addr = self.peer_addr()?;
        Ok((n, addr))
    }

    fn read(
        &self,
        buf: &mut [u8],
        _offset: usize,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        self.recv(buf, nonblocking, sleep)
    }

    fn sendto(
        &self,
        buf: &[u8],
        _dest: Option<SockAddr>,
        _flags: c_int,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        // TODO: Handle dest and flags
        if buf.is_empty() {
            return Ok(0);
        }

        let sleep_guard = sleep.guard(&self.wait_queue)?;
        loop {
            let mut mutable = self.mutable.lock();
            if let Some(error) = mutable.error.take() {
                return Err(error);
            }

            if mutable.write_closed {
                return Err(Errno::EPIPE);
            }

            if mutable.state != State::SynSent {
                if mutable.state != State::Established && mutable.state != State::CloseWait {
                    // The connection is not in a writable state.
                    return Err(Errno::EPIPE);
                }

                let written_len = mutable.tx_buffer.write(buf).map_err(Errno::from)?;
                if written_len > 0 {
                    self.flush(&mut mutable);
                    return Ok(written_len);
                }
            }

            if nonblocking {
                return Err(Errno::EAGAIN);
            }

            if sleep_guard.is_interrupted() {
                return Err(Errno::EINTR);
            }

            drop(mutable);
            sleep_guard.wait()?;
        }
    }

    fn write(
        &self,
        buf: &[u8],
        _offset: usize,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        self.sendto(buf, None, 0, nonblocking, sleep)
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

    fn shutdown(&self, how: c_int) -> Result<(), Errno> {
        self.do_shutdown(how)
    }

    fn poll(&self) -> Result<c_short, Errno> {
        let mut status = 0;
        let mutable = self.mutable.lock();

        // This is readable if read() would return 0 anyway (EOF/closed).
        if !mutable.rx_buffer.is_empty()
            || mutable.eof
            || mutable.read_closed
            || mutable.state == State::Closed
        {
            status |= POLLIN;
        }

        if !mutable.write_closed
            && (mutable.state == State::Established || mutable.state == State::CloseWait)
            && mutable.tx_buffer.writable_len() > 0
        {
            status |= POLLOUT;
        }

        if mutable.error.is_some() {
            status |= POLLERR | POLLOUT;
        }

        Ok(status)
    }

    fn wait_queue(&self) -> Option<&WaitQueue> {
        Some(&self.wait_queue)
    }
}
