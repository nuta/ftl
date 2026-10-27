use alloc::sync::Arc;
use core::cmp::min;

use crate::net::TcpConn;
use crate::types::c_int;
use crate::types::c_short;
use crate::types::errno::Errno;
use crate::types::sys::poll::POLLOUT;
use crate::types::sys::uio::IoVec;
use crate::vfs::FileLike;
use crate::vfs::IoVecSlice;
use crate::wait_queue::Sleep;

const FRAME_PAYLOAD_MAX: usize = 2048;

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum FrameKind {
    Stdout = 1,
    Stderr = 2,
    Exit = 3,
}

pub struct StdioConn(Arc<TcpConn>);

impl StdioConn {
    pub fn new(conn: Arc<TcpConn>) -> Self {
        Self(conn)
    }

    pub fn send_exit(&self, status: c_int) -> Result<(), Errno> {
        let [status0, status1] = (status as u16).to_le_bytes();
        let header = [FrameKind::Exit as u8, status0, status1];
        let empty: [IoVec; 0] = [];
        let iovecs = IoVecSlice::new(empty.as_ptr(), 0);
        self.0
            .write_atomic(&header, &iovecs, 0, true, Sleep::Uninterruptible)
    }

    pub fn close(&self) {
        self.0.close();
    }

    pub fn is_peer_closed(&self) -> bool {
        self.0.is_peer_closed()
    }
}

impl Drop for StdioConn {
    fn drop(&mut self) {
        self.0.close();
    }
}

pub struct StdioStream {
    conn: Arc<StdioConn>,
    kind: FrameKind,
}

impl StdioStream {
    pub fn new(conn: Arc<StdioConn>, kind: FrameKind) -> Self {
        Self { conn, kind }
    }
}

impl FileLike for StdioStream {
    fn write(
        &self,
        buf: &[u8],
        offset: usize,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        let iovec = IoVec {
            iov_base: buf.as_ptr().cast_mut().cast(),
            iov_len: buf.len(),
        };

        self.writev(&IoVecSlice::new(&iovec, 1), offset, nonblocking, sleep)
    }

    fn writev(
        &self,
        iovecs: &IoVecSlice,
        _offset: usize,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        let len = min(iovecs.total_len(), FRAME_PAYLOAD_MAX);
        if len == 0 {
            return Ok(0);
        }

        let [len0, len1] = (len as u16).to_le_bytes();
        let header = [self.kind as u8, len0, len1];
        self.conn
            .0
            .write_atomic(&header, iovecs, len, nonblocking, sleep)?;

        Ok(len)
    }

    fn poll(&self) -> Result<c_short, Errno> {
        Ok(POLLOUT)
    }
}
