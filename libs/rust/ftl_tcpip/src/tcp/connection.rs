use alloc::collections::VecDeque;
use core::fmt;

use crate::Io;
use crate::socket::AnySocket;

#[derive(Debug)]
enum State {
    Listen,
    Established,
    FinWait1,
    FinWait2,
    Closing,
}

struct Mutable<I: Io> {
    state: State,
    pending_writes: VecDeque<I::TcpWrite>,
    pending_reads: VecDeque<I::TcpRead>,
}

pub struct TcpConn<I: Io> {
    mutable: spin::Mutex<Mutable<I>>,
}

impl<I: Io> AnySocket for TcpConn<I> {}

impl<I: Io> fmt::Debug for TcpConn<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpConn").finish()
    }
}
