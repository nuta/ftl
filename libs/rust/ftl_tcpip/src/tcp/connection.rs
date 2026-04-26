use alloc::collections::VecDeque;
use core::cmp::min;
use core::fmt;

use super::ring_buffer::RingBuffer;
use crate::Io;
use crate::device::DeviceMap;
use crate::route::RouteTable;
use crate::socket::AnySocket;
use crate::socket::Endpoint;
use crate::tcp::Read;
use crate::tcp::Write;
use crate::tcp::header::TcpFlags;
use crate::tcp::header::TcpHeader;
use crate::tcp::tx::transmit_segment;
use crate::transport::Port;

pub(super) const DEFAULT_RCV_WND: u16 = 1024;

#[derive(Debug)]
enum State {
    Listen,
    Established,
    FinWait1,
    FinWait2,
    Closing,
}

//           snd_una        snd_nxt
//             |               |
// --[ACKed] --+---[inflight]--+---[max_bytes]--+---[cannot send]------
//             |<-- snd_wnd ------------------->|
struct Mutable<I: Io> {
    state: State,
    remote: Option<Endpoint>,
    local_port: Port,
    /// Sequence number of the first byte not yet acknowledged by the peer.
    snd_una: u32,
    /// Sequence number of the next byte to send.
    snd_nxt: u32,
    /// Peer's receive window size: how much we can send. Fullfilled when the
    /// peer sends an ACK.
    snd_wnd: u16,
    /// Sequence number of the next byte we expect to receive.
    rcv_nxt: u32,
    /// Our receive window size. How much RX buffer space we have.
    rcv_wnd: u16,
    tx_buffer: RingBuffer,
    rx_buffer: RingBuffer,
    pending_writes: VecDeque<I::TcpWrite>,
    pending_reads: VecDeque<I::TcpRead>,
}

pub struct TcpConn<I: Io> {
    mutable: spin::Mutex<Mutable<I>>,
}

impl<I: Io> TcpConn<I> {
    pub(crate) fn new_listen() -> Self {
        Self {
            mutable: spin::Mutex::new(Mutable {
                state: State::Listen,
                remote: None,
                local_port: Port::new(0), // FIXME:
                snd_una: 0,
                snd_nxt: 0,
                snd_wnd: 0,
                rcv_nxt: 0,
                rcv_wnd: 0,
                tx_buffer: RingBuffer::new(),
                rx_buffer: RingBuffer::new(),
                pending_writes: VecDeque::new(),
                pending_reads: VecDeque::new(),
            }),
        }
    }

    pub(crate) fn open_passively(&self, remote: Endpoint, iss: u32, rcv_nxt: u32, snd_wnd: u16) {
        let snd_nxt = iss.wrapping_add(1); // +1 for the SYN packet
        let mut mutable = self.mutable.lock();
        mutable.state = State::Established;
        mutable.remote = Some(remote);
        mutable.snd_una = snd_nxt;
        mutable.snd_nxt = snd_nxt;
        mutable.snd_wnd = snd_wnd;
        mutable.rcv_nxt = rcv_nxt;
        mutable.rcv_wnd = DEFAULT_RCV_WND;
    }

    pub fn write(&self, req: I::TcpWrite) {
        let mut mutable = self.mutable.lock();
        if mutable.tx_buffer.writeable_len() == 0 {
            mutable.pending_writes.push_back(req);
        } else {
            req.complete(&mut mutable.tx_buffer);
            if mutable.tx_buffer.readable_len() > 0 {
                todo!("send data from buffer");
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

    fn handle_rx(&self) {
        let mut mutable = self.mutable.lock();
    }

    fn flush(
        &self,
        devices: &mut DeviceMap<I::Device>,
        routes: &mut RouteTable,
        mutable: &mut Mutable<I>,
    ) {
        match &mut mutable.state {
            State::Established => {
                let inflight = mutable.snd_nxt.wrapping_sub(mutable.snd_una) as usize;
                let max_len = (mutable.snd_wnd as usize) - inflight;
                let remote = mutable.remote.as_ref().unwrap();
                if let Some(payload) = mutable.tx_buffer.peek_bytes(max_len) {
                    let header = TcpHeader {
                        src_port: mutable.local_port.into(),
                        dst_port: remote.port.into(),
                        seq: mutable.snd_nxt.into(),
                        ack: mutable.rcv_nxt.into(),
                        window_size: mutable.rcv_wnd.into(),
                        flags: TcpFlags::ACK | TcpFlags::PSH,
                        header_len: 0,
                        checksum: 0.into(),
                        urgent_pointer: 0.into(),
                    };

                    trace!("TCP: sending {} bytes", payload.len());
                    if let Err(err) =
                        transmit_segment::<I>(devices, routes, header, remote.addr, payload)
                    {
                        warn!("TCP: failed to send data: {:?}", err);
                        return;
                    }

                    let payload_len = payload.len();
                    mutable.snd_nxt = mutable.snd_nxt.wrapping_add(payload_len as u32);
                    mutable.tx_buffer.consume_bytes(payload_len);
                }
            }
            State::Listen => {}
            State::FinWait1 | State::FinWait2 | State::Closing => {}
        }
    }
}

impl<I: Io> AnySocket for TcpConn<I> {}

impl<I: Io> fmt::Debug for TcpConn<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpConn").finish()
    }
}
