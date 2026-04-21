use alloc::collections::VecDeque;

use crate::socket::AnySocket;

pub trait ReadRequest {
    type Error;

    fn write(self, buf: &[u8]) -> Result<(), Self::Error>;
    fn error(self, error: Self::Error) -> Result<(), Self::Error>;
    fn complete(self, read_len: usize) -> Result<(), Self::Error>;
}

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

pub struct TcpConn<WriteR: WriteRequest, ReadR: ReadRequest> {
    pending_writes: VecDeque<WriteR>,
    pending_reads: VecDeque<ReadR>,
}

impl<WriteR: WriteRequest, ReadR: ReadRequest> TcpConn<WriteR, ReadR> {
    pub fn new() -> Self {
        Self {
            pending_writes: VecDeque::new(),
            pending_reads: VecDeque::new(),
        }
    }
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
