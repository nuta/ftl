use alloc::collections::VecDeque;
use core::fmt;

use crate::Io;
use crate::socket::{AnySocket, Endpoint};

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
                snd_una: 0,
                snd_nxt: 0,
                snd_wnd: 0,
                rcv_nxt: 0,
                rcv_wnd: 0,
                pending_writes: VecDeque::new(),
                pending_reads: VecDeque::new(),
            }),
        }
    }

    pub(crate) fn open_passively(&self,
        remote: Endpoint,
        iss: u32,
        rcv_nxt: u32,
        snd_wnd: u16,
    ) {
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
}

impl<I: Io> AnySocket for TcpConn<I> {}

impl<I: Io> fmt::Debug for TcpConn<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpConn").finish()
    }
}
