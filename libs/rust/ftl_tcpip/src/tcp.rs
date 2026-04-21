use alloc::collections::VecDeque;

use crate::{Io, socket::AnySocket};

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

pub struct TcpConn<I: Io> {
    pending_writes: VecDeque<I::TcpWrite>,
    pending_reads: VecDeque<I::TcpRead>,
}

impl<I: Io> TcpConn<I> {
    pub fn new() -> Self {
        Self {
            pending_writes: VecDeque::new(),
            pending_reads: VecDeque::new(),
        }
    }
}

pub struct TcpListener<I: Io> {
    pending_accepts: VecDeque<I::TcpAccept>,
}

impl<I: Io> TcpListener<I> {
    pub fn new() -> Self {
        Self {
            pending_accepts: VecDeque::new(),
        }
    }

    pub fn accept(&mut self) -> Result<I::TcpAccept, ()> {
        self.pending_accepts.pop_front().ok_or(())
    }
}

impl<I: Io> AnySocket for TcpListener<I> {}
