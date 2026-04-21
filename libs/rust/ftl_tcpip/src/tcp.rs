use alloc::collections::VecDeque;

use crate::socket::AnySocket;

pub trait WriteRequest {
    type Error;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;
    fn error(self, error: Self::Error) -> Result<(), Self::Error>;
    fn complete(self, written_len: usize) -> Result<(), Self::Error>;
}

pub trait AcceptRequest: Send + Sync + Sized + 'static {
    type Error;

    fn complete(self) -> Result<(), Self::Error>;
}

pub struct TcpListener<AcceptR: AcceptRequest> {
    pending_accepts: VecDeque<AcceptR>,
}

impl<AcceptR: AcceptRequest> TcpListener<AcceptR> {
    pub fn new() -> Self {
        Self {
            pending_accepts: VecDeque::new(),
        }
    }

    pub fn accept(&mut self) -> Result<AcceptR, ()> {
        self.pending_accepts.pop_front().ok_or(())
    }
}

impl<AcceptR: AcceptRequest> AnySocket for TcpListener<AcceptR> {}
