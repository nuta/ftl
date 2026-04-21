use alloc::collections::VecDeque;

use crate::{Io, OutOfMemoryError};
use crate::socket::AnySocket;

pub trait Read {
    type Error;

    fn write(self, buf: &[u8]) -> Result<(), Self::Error>;
    fn error(self, error: Self::Error) -> Result<(), Self::Error>;
    fn complete(self, read_len: usize) -> Result<(), Self::Error>;
}

pub trait Write {
    type Error;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;
    fn error(self, error: Self::Error) -> Result<(), Self::Error>;
    fn complete(self, written_len: usize) -> Result<(), Self::Error>;
}

pub trait Accept: Send + Sync + Sized + 'static {
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

    pub fn accept(&mut self, req: I::TcpAccept) -> Result<(), OutOfMemoryError> {
        self.pending_accepts.try_reserve(1).map_err(|_| OutOfMemoryError)?;
        self.pending_accepts.push_back(req);
        Ok(())
    }
}

impl<I: Io> AnySocket for TcpListener<I> {}
