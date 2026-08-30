use alloc::sync::Arc;
use alloc::sync::Weak;
use core::cmp::min;

use ftl::syscall::poll_notify;
use ftl::syscall::poll_wait;
use ftl_types::handle::HandleId;
use ftl_utils::spinlock::SpinLock;

use super::buffer::TcpBuffer;
use super::packet::Endpoint;
use super::packet::Segment;
use super::packet::TcpFlags;
use super::packet::TcpSegmentMeta;
use crate::net::TcpIp;
use crate::types::errno::Errno;
use crate::vfs::FileLike;

const MAX_SEGMENT_DATA_LEN: usize = 1460;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConnectionState {
    Established,
    CloseWait,
    LastAck,
    FinWait1,
    FinWait2,
    Closing,
    Closed,
}

struct Mutable {
    state: ConnectionState,
    close_requested: bool,
    snd_una: u32,
    snd_nxt: u32,
    snd_wnd: u16,
    rcv_nxt: u32,
    tx_buffer: TcpBuffer,
    rx_buffer: TcpBuffer,
    eof: bool,
}

pub struct TcpConnection {
    network: Weak<TcpIp>,
    poll: HandleId,
    remote: Endpoint,
    local_port: u16,
    mutable: SpinLock<Mutable>,
}

impl TcpConnection {
    pub(super) fn new(
        network: Weak<TcpIp>,
        poll: HandleId,
        remote: Endpoint,
        local_port: u16,
        local_iss: u32,
        remote_rcv_nxt: u32,
        remote_rcv_wnd: u16,
        rx_buffer: TcpBuffer,
    ) -> Arc<Self> {
        let snd_nxt = local_iss.wrapping_add(1);
        let mutable = Mutable {
            state: ConnectionState::Established,
            close_requested: false,
            snd_una: snd_nxt,
            snd_nxt,
            snd_wnd: remote_rcv_wnd,
            rcv_nxt: remote_rcv_nxt,
            tx_buffer: TcpBuffer::new(),
            rx_buffer,
            eof: false,
        };
        Arc::new(Self {
            network,
            poll,
            remote,
            local_port,
            mutable: SpinLock::new(mutable),
        })
    }

    pub fn matches(&self, info: &TcpSegmentMeta) -> bool {
        if self.remote.ip != info.remote_ip || self.remote.port != info.remote_port {
            return false;
        }
        self.local_port == info.local_port
    }

    pub fn is_closed(&self) -> bool {
        self.mutable.lock().state == ConnectionState::Closed
    }

    fn send(&self, segment: Segment) -> Result<(), Errno> {
        let Some(network) = self.network.upgrade() else {
            return Err(Errno::EINVAL);
        };
        network
            .send_segment(self.remote, self.local_port, segment)
            .map_err(Errno::from)
    }

    fn acknowledge(&self, mutable: &Mutable) {
        let window_size = mutable.rx_buffer.writable_len() as u16;
        let segment = Segment {
            seq: mutable.snd_nxt,
            ack: mutable.rcv_nxt,
            window_size,
            flags: TcpFlags::ACK,
            payload: &[],
        };
        let _ = self.send(segment);
    }

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
            ConnectionState::FinWait1 => ConnectionState::FinWait2,
            ConnectionState::Closing | ConnectionState::LastAck => ConnectionState::Closed,
            state => state,
        };
        Some(acked_len)
    }

    fn receive_payload(&self, mutable: &mut Mutable, payload_len: usize) -> Option<(bool, bool)> {
        let network = self.network.upgrade()?;
        let consumed = mutable
            .rx_buffer
            .receive(payload_len, |payload| network.recv_payload(payload))
            .ok()?;
        if !consumed {
            return Some((false, false));
        }
        mutable.rcv_nxt = mutable.rcv_nxt.wrapping_add(payload_len as u32);
        Some((true, payload_len != 0))
    }

    fn receive_fin(&self, mutable: &mut Mutable, info: &TcpSegmentMeta) -> bool {
        let flags = TcpFlags::from_u8(info.flags);
        if !flags.contains(TcpFlags::FIN) {
            return false;
        }

        let fin_seq = info.seq.wrapping_add(info.payload_len as u32);
        if fin_seq != mutable.rcv_nxt {
            return false;
        }

        mutable.rcv_nxt = mutable.rcv_nxt.wrapping_add(1);
        mutable.eof = true;
        mutable.state = match mutable.state {
            ConnectionState::Established => ConnectionState::CloseWait,
            ConnectionState::FinWait1 => ConnectionState::Closing,
            ConnectionState::FinWait2 => ConnectionState::Closed,
            state => state,
        };
        true
    }

    fn flush(&self, mutable: &mut Mutable) {
        if mutable.state != ConnectionState::Established
            && mutable.state != ConnectionState::CloseWait
        {
            return;
        }

        if mutable.close_requested && mutable.tx_buffer.is_empty() {
            self.send_fin(mutable);
            return;
        }

        let inflight_len = mutable.snd_nxt.wrapping_sub(mutable.snd_una) as usize;
        let window_len = mutable.snd_wnd as usize;
        let sendable_len = window_len.saturating_sub(inflight_len);
        let sendable_len = min(sendable_len, MAX_SEGMENT_DATA_LEN);
        let Some(payload) = mutable.tx_buffer.peek_from(inflight_len, sendable_len) else {
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

        // Keep the state locked until net_send returns. This makes snd_nxt advance
        // only after the kernel accepted the segment.
        if self.send(segment).is_ok() {
            mutable.snd_nxt = mutable.snd_nxt.wrapping_add(payload_len as u32);
        }
    }

    fn send_fin(&self, mutable: &mut Mutable) {
        let window_size = mutable.rx_buffer.writable_len() as u16;
        let segment = Segment {
            seq: mutable.snd_nxt,
            ack: mutable.rcv_nxt,
            window_size,
            flags: TcpFlags::FIN | TcpFlags::ACK,
            payload: &[],
        };
        if self.send(segment).is_err() {
            return;
        }

        mutable.snd_nxt = mutable.snd_nxt.wrapping_add(1);
        mutable.state = match mutable.state {
            ConnectionState::Established => ConnectionState::FinWait1,
            ConnectionState::CloseWait => ConnectionState::LastAck,
            state => state,
        };
    }

    pub fn handle_packet(&self, info: &TcpSegmentMeta) -> bool {
        let flags = TcpFlags::from_u8(info.flags);
        let mut mutable = self.mutable.lock();
        mutable.snd_wnd = info.window_size;

        if flags.contains(TcpFlags::RST) {
            mutable.state = ConnectionState::Closed;
            mutable.eof = true;
            drop(mutable);
            let _ = poll_notify(self.poll);
            return false;
        }

        let acked_len = if flags.contains(TcpFlags::ACK) {
            let Some(acked_len) = self.handle_ack(&mut mutable, info.ack) else {
                return false;
            };
            acked_len
        } else {
            0
        };

        if info.seq != mutable.rcv_nxt {
            self.acknowledge(&mutable);
            drop(mutable);
            if acked_len > 0 {
                let _ = poll_notify(self.poll);
            }
            return false;
        }

        let Some((consumed, received_payload)) =
            self.receive_payload(&mut mutable, info.payload_len as usize)
        else {
            return false;
        };
        if !consumed {
            self.acknowledge(&mutable);
            drop(mutable);
            if acked_len > 0 {
                let _ = poll_notify(self.poll);
            }
            return false;
        }
        let received_fin = self.receive_fin(&mut mutable, info);
        if received_payload || received_fin {
            self.acknowledge(&mutable);
        }
        self.flush(&mut mutable);
        drop(mutable);

        if acked_len > 0 || received_payload || received_fin {
            let _ = poll_notify(self.poll);
        }
        true
    }

    pub fn close(&self) {
        let mut mutable = self.mutable.lock();
        mutable.close_requested = true;
        self.flush(&mut mutable);
    }
}

impl FileLike for TcpConnection {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn read(&self, output: &mut [u8], _offset: usize) -> Result<usize, Errno> {
        loop {
            let mut mutable = self.mutable.lock();
            if !mutable.rx_buffer.is_empty() {
                let len = mutable.rx_buffer.read(output);
                self.acknowledge(&mutable);
                return Ok(len);
            }
            if mutable.eof || mutable.state == ConnectionState::Closed {
                return Ok(0);
            }

            drop(mutable);
            poll_wait(self.poll)?;
        }
    }

    fn write(&self, input: &[u8], _offset: usize) -> Result<usize, Errno> {
        loop {
            let mut mutable = self.mutable.lock();
            if mutable.state != ConnectionState::Established
                && mutable.state != ConnectionState::CloseWait
            {
                return Err(Errno::EINVAL);
            }

            let written_len = mutable.tx_buffer.write(input);
            if written_len > 0 || input.is_empty() {
                self.flush(&mut mutable);
                return Ok(written_len);
            }

            drop(mutable);
            poll_wait(self.poll)?;
        }
    }
}
