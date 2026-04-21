use alloc::collections::VecDeque;

use crate::Io;
use crate::OutOfMemoryError;
use crate::socket::AnySocket;
use crate::utils::TryPushBack;

pub enum Error {}

pub trait Read: Send + Sync {
    fn write(&mut self, buf: &[u8]) -> usize;
    fn complete(self, result: Result<usize, Error>);
}

pub trait Write: Send + Sync {
    fn read(&mut self, buf: &mut [u8]) -> usize;
    fn complete(self, result: Result<usize, Error>);
}

pub trait Accept: Send + Sync {
    fn complete(self, result: Result<(), Error>);
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

impl<I: Io> AnySocket for TcpConn<I> {}

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
        self.pending_accepts.try_push_back(req)?;
        Ok(())
    }
}

impl<I: Io> AnySocket for TcpListener<I> {}
