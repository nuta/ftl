use alloc::collections::VecDeque;
use core::cmp::min;
use core::fmt;

use super::ring_buffer::RingBuffer;
use crate::Io;
use crate::device::DeviceMap;
use crate::packet::Packet;
use crate::route::RouteTable;
use crate::socket::AnySocket;
use crate::socket::Endpoint;
use crate::tcp::Read;
use crate::tcp::Write;
use crate::tcp::header::TcpFlags;
use crate::tcp::header::TcpHeader;
use crate::tcp::rx::RxHeader;
use crate::tcp::tx::transmit_segment;
use crate::transport::Port;

pub(super) const DEFAULT_RCV_WND: u16 = 1024;
const MAX_SEGMENT_DATA_LEN: usize = 1460;

#[derive(Debug, Clone, Copy)]
enum State {
    Listen,
    Established,
    CloseWait,
    LastAck,
    FinWait1,
    FinWait2,
    Closing,
    TimeWait,
    Closed,
}

//           snd_una        snd_nxt
//             |               |
// --[ACKed] --+---[inflight]--+---[max_bytes]--+---[cannot send]------
//             |<-- snd_wnd ------------------->|
struct Mutable<I: Io> {
    state: State,
    remote: Option<Endpoint>,
    local_port: Port,
    close_requested: bool,
    /// Sequence number of the first byte not yet acknowledged by the peer.
    snd_una: u32,
    /// Sequence number of the next byte to send.
    snd_nxt: u32,
    /// Peer's receive window size: how much we can send. Fullfilled when the
    /// peer sends an ACK.
    snd_wnd: u16,
    /// Sequence number of the next byte we expect to receive.
    rcv_nxt: u32,
    tx_buffer: RingBuffer,
    rx_buffer: RingBuffer,
    pending_writes: VecDeque<I::TcpWrite>,
    pending_reads: VecDeque<I::TcpRead>,
}

pub struct TcpConn<I: Io> {
    mutable: spin::Mutex<Mutable<I>>,
}

impl<I: Io> TcpConn<I> {
    pub(crate) fn new_listen(local_port: Port) -> Self {
        Self {
            mutable: spin::Mutex::new(Mutable {
                state: State::Listen,
                remote: None,
                local_port,
                close_requested: false,
                snd_una: 0,
                snd_nxt: 0,
                snd_wnd: 0,
                rcv_nxt: 0,
                tx_buffer: RingBuffer::new(),
                rx_buffer: RingBuffer::new(),
                pending_writes: VecDeque::new(),
                pending_reads: VecDeque::new(),
            }),
        }
    }

    pub(crate) fn open_passively(
        &self,
        remote: Endpoint,
        iss: u32,
        rcv_nxt: u32,
        snd_wnd: u16,
        rx_buffer: RingBuffer,
    ) {
        let snd_nxt = iss.wrapping_add(1); // +1 for the SYN packet
        let mut mutable = self.mutable.lock();
        mutable.state = State::Established;
        mutable.remote = Some(remote);
        mutable.snd_una = snd_nxt;
        mutable.snd_nxt = snd_nxt;
        mutable.snd_wnd = snd_wnd;
        mutable.rcv_nxt = rcv_nxt;
        mutable.rx_buffer = rx_buffer;
    }

    pub fn write(
        &self,
        devices: &mut DeviceMap<I::Device>,
        routes: &mut RouteTable,
        req: I::TcpWrite,
    ) {
        let mut mutable = self.mutable.lock();
        if mutable.tx_buffer.writeable_len() == 0 {
            mutable.pending_writes.push_back(req);
        } else {
            req.complete(&mut mutable.tx_buffer);
            if mutable.tx_buffer.readable_len() > 0 {
                self.flush(devices, routes, &mut mutable);
            }
        }
    }

    pub fn read(&self, req: I::TcpRead) {
        let mut mutable = self.mutable.lock();
        if mutable.rx_buffer.is_empty() {
            mutable.pending_reads.push_back(req);
        } else {
            req.complete(&mut mutable.rx_buffer);
        }
    }

    pub fn close(&self, devices: &mut DeviceMap<I::Device>, routes: &mut RouteTable) {
        let mut mutable = self.mutable.lock();
        mutable.close_requested = true;
        self.flush(devices, routes, &mut mutable);
    }

    pub(super) fn handle_rx(
        &self,
        devices: &mut DeviceMap<I::Device>,
        routes: &mut RouteTable,
        rx: RxHeader,
        payload: &mut Packet,
    ) {
        let mut mutable = self.mutable.lock();

        if rx.seq != mutable.rcv_nxt {
            trace!(
                "TCP: out-of-order data: seq={}, rcv_nxt={}",
                rx.seq, mutable.rcv_nxt
            );
            self.send_ack(devices, routes, &mutable);
            return;
        }

        mutable.snd_wnd = rx.window_size;

        // Handle ACK: remote have received data from us.
        if rx.flags.contains(TcpFlags::ACK) {
            let acked_bytes = rx.ack.wrapping_sub(mutable.snd_una) as usize;
            let inflight_bytes = mutable.snd_nxt.wrapping_sub(mutable.snd_una) as usize;
            if acked_bytes > inflight_bytes {
                debug!(
                    "TCP: ACKed more bytes than in flight: acked={}, inflight={}",
                    acked_bytes, inflight_bytes
                );
                return;
            }

            if acked_bytes > 0 {
                // The remote has acknowledged some bytes. Consume them from
                // the write buffer, and read more data from the pending
                // writes.
                mutable.snd_una = rx.ack;
                mutable.tx_buffer.consume_bytes(acked_bytes);
                while mutable.tx_buffer.writeable_len() > 0 {
                    if let Some(req) = mutable.pending_writes.pop_front() {
                        req.complete(&mut mutable.tx_buffer);
                    } else {
                        break;
                    }
                }
            }

            // Is the remote has acknowledged all data we have sent so far?
            if mutable.snd_una == mutable.snd_nxt {
                // FIN state transitions: the remote acknowledged our FIN.
                match mutable.state {
                    State::FinWait1 => mutable.state = State::FinWait2,
                    State::Closing => mutable.state = State::TimeWait,
                    State::LastAck => mutable.state = State::Closed,
                    _ => {}
                }
            }
        }

        // Handle payload: remote have sent data to us.
        let payload = payload.slice();
        let mut should_ack = false;
        if !payload.is_empty() {
            // Receive data from the remote.
            should_ack = true;
            let written_len = mutable.rx_buffer.write_bytes(payload);
            mutable.rcv_nxt = mutable.rcv_nxt.wrapping_add(written_len as u32);

            while mutable.rx_buffer.readable_len() > 0 {
                // We have data in the receive buffer. Try consuming them.
                if let Some(req) = mutable.pending_reads.pop_front() {
                    req.complete(&mut mutable.rx_buffer);
                } else {
                    break;
                }
            }
        }

        // Handle FIN: remote wants to close the connection.
        if rx.flags.contains(TcpFlags::FIN) {
            mutable.rcv_nxt = mutable.rcv_nxt.wrapping_add(1);
            should_ack = true;

            match mutable.state {
                // The remote closed first. Keep the send side open until we
                // close the connection (passive close).
                State::Established => mutable.state = State::CloseWait,
                // Simultaneous close; wait for the remote to ACK our FIN.
                State::FinWait1 => mutable.state = State::Closing,
                // The remote acknowledged our FIN. Acknowledge their FIN, and
                // enter TIME_WAIT state.
                State::FinWait2 => mutable.state = State::TimeWait,
                _ => {}
            }
        }

        if should_ack {
            self.send_ack(devices, routes, &mutable);
        }

        self.flush(devices, routes, &mut mutable);
    }

    fn send_ack(
        &self,
        devices: &mut DeviceMap<I::Device>,
        routes: &mut RouteTable,
        mutable: &Mutable<I>,
    ) {
        let Some(remote) = mutable.remote.as_ref() else {
            return;
        };

        let header = TcpHeader {
            src_port: mutable.local_port.into(),
            dst_port: remote.port.into(),
            seq: mutable.snd_nxt.into(),
            ack: mutable.rcv_nxt.into(),
            window_size: (mutable.rx_buffer.writeable_len() as u16).into(),
            flags: TcpFlags::ACK,
            header_len: 0.into(),
            checksum: 0.into(),
            urgent_pointer: 0.into(),
        };

        if let Err(err) = transmit_segment::<I>(devices, routes, header, remote.addr, &[]) {
            warn!("TCP: failed to send ACK: {:?}", err);
        }
    }

    fn flush(
        &self,
        devices: &mut DeviceMap<I::Device>,
        routes: &mut RouteTable,
        mutable: &mut Mutable<I>,
    ) {
        match mutable.state {
            State::Established | State::CloseWait => {
                let remote = mutable.remote.as_ref().unwrap();
                let seq = mutable.snd_nxt.into();

                let mut flags = TcpFlags::empty();
                let payload = if mutable.close_requested && mutable.tx_buffer.readable_len() == 0 {
                    trace!("TCP: sending FIN");
                    flags |= TcpFlags::FIN | TcpFlags::ACK;
                    mutable.snd_nxt = mutable.snd_nxt.wrapping_add(1);
                    mutable.state = match mutable.state {
                        // Active close.
                        State::Established => State::FinWait1,
                        // Passive close after the local user closes its send side.
                        State::CloseWait => State::LastAck,
                        _ => unreachable!(),
                    };

                    &[] // No data to send in payload.
                } else {
                    let inflight_len = mutable.snd_nxt.wrapping_sub(mutable.snd_una) as usize;
                    let sendable_len = min(
                        (mutable.snd_wnd as usize).saturating_sub(inflight_len),
                        MAX_SEGMENT_DATA_LEN,
                    );
                    let Some(payload) = mutable
                        .tx_buffer
                        .peek_bytes_from(inflight_len, sendable_len)
                    else {
                        // No data to send.
                        return;
                    };

                    trace!("TCP: sending data: len={}", payload.len());
                    flags |= TcpFlags::ACK | TcpFlags::PSH;
                    mutable.snd_nxt = mutable.snd_nxt.wrapping_add(payload.len() as u32);
                    payload
                };

                let header = TcpHeader {
                    src_port: mutable.local_port.into(),
                    dst_port: remote.port.into(),
                    seq,
                    ack: mutable.rcv_nxt.into(),
                    window_size: (mutable.rx_buffer.writeable_len() as u16).into(),
                    flags,
                    header_len: 0,
                    checksum: 0.into(),
                    urgent_pointer: 0.into(),
                };

                if let Err(err) =
                    transmit_segment::<I>(devices, routes, header, remote.addr, payload)
                {
                    warn!("TCP: failed to send data: {:?}", err);
                    return;
                }
            }
            State::Listen => {
                unreachable!();
            }
            State::LastAck
            | State::FinWait1
            | State::FinWait2
            | State::Closing
            | State::TimeWait
            | State::Closed => {}
        }
    }
}

impl<I: Io> AnySocket for TcpConn<I> {}

impl<I: Io> fmt::Debug for TcpConn<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpConn").finish()
    }
}
