use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::min;

use ftl::trace;
use ftl_utils::spinlock::SpinLock;

use crate::types::c_short;
use crate::types::errno::Errno;
use crate::types::sys::poll::POLLERR;
use crate::types::sys::poll::POLLHUP;
use crate::types::sys::poll::POLLIN;
use crate::types::sys::poll::POLLOUT;
use crate::types::sys::uio::IoVec;
use crate::vfs::FileLike;
use crate::vfs::IoVecSlice;
use crate::wait_queue::Sleep;
use crate::wait_queue::WaitQueue;

/// The maximum size of a write to the pipe which is guaranteed to be atomic,
/// and 4096 is what Linux uses.
const PIPE_BUF: usize = 4096;
/// The pipe capacity.
const PIPE_CAPACITY: usize = 8192;

#[derive(Clone, Copy, PartialEq, Eq)]
enum End {
    Read,
    Write,
}

struct Mutable {
    buffer: Vec<u8>,
    reader_open: bool,
    writer_open: bool,
}

struct Inner {
    mutable: SpinLock<Mutable>,
    wait_queue: WaitQueue,
}

pub struct Pipe {
    inner: Arc<Inner>,
    end: End,
}

impl Pipe {
    pub fn pair() -> Result<(Self, Self), Errno> {
        let mut buffer = Vec::new();
        buffer
            .try_reserve_exact(PIPE_CAPACITY)
            .map_err(|_| Errno::ENOMEM)?;

        let inner = Arc::new(Inner {
            mutable: SpinLock::new(Mutable {
                buffer,
                reader_open: true,
                writer_open: true,
            }),
            wait_queue: WaitQueue::new()?,
        });

        Ok((
            Self {
                inner: inner.clone(),
                end: End::Read,
            },
            Self {
                inner,
                end: End::Write,
            },
        ))
    }

    fn notify(&self) {
        if let Err(error) = self.inner.wait_queue.notify_all() {
            trace!("failed to notify pipe waiters: {:?}", error);
        }
    }
}

impl FileLike for Pipe {
    fn read(
        &self,
        buf: &mut [u8],
        _offset: usize,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        if self.end != End::Read {
            return Err(Errno::EBADF);
        }

        let sleep_guard = sleep.guard(&self.inner.wait_queue)?;
        loop {
            let mut mutable = self.inner.mutable.lock();
            if !mutable.buffer.is_empty() {
                // Read the data from the pipe.
                let n = min(buf.len(), mutable.buffer.len());
                buf[..n].copy_from_slice(&mutable.buffer[..n]);

                // Remove the read data from the pipe.
                mutable.buffer.drain(..n);
                drop(mutable);

                // Notify the waiters. There might be writers that are waiting
                // for the pipe to be writable.
                self.notify();
                return Ok(n);
            }

            if !mutable.writer_open {
                // The pipe is empty and the writer is closed. EOF.
                return Ok(0);
            }

            if nonblocking {
                return Err(Errno::EAGAIN);
            }

            if sleep_guard.is_interrupted() {
                return Err(Errno::EINTR);
            }

            // Wait for the writer to write data to the pipe.
            drop(mutable);
            sleep_guard.wait()?;
        }
    }

    // TODO: Use writev only.
    fn write(
        &self,
        buf: &[u8],
        offset: usize,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        let iovec = IoVec {
            iov_base: buf.as_ptr().cast_mut().cast(), // FIXME:
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
        if self.end != End::Write {
            return Err(Errno::EBADF);
        }

        let total_len = iovecs.total_len();
        if total_len == 0 {
            return Ok(0);
        }

        let atomic = total_len <= PIPE_BUF;

        // Wait until the pipe is writable.
        let sleep_guard = sleep.guard(&self.inner.wait_queue)?;
        let (mut mutable, writeable_len) = loop {
            let mutable = self.inner.mutable.lock();
            if !mutable.reader_open {
                return Err(Errno::EPIPE);
            }

            // If len <= PIPE_BUF, we need to write the entire buffer
            // atomically.
            let writeable_len = PIPE_CAPACITY.saturating_sub(mutable.buffer.len());
            if writeable_len == 0 || (atomic && writeable_len < total_len) {
                if nonblocking {
                    return Err(Errno::EAGAIN);
                }

                if sleep_guard.is_interrupted() {
                    return Err(Errno::EINTR);
                }

                drop(mutable);
                sleep_guard.wait()?;
                continue;
            }

            break (mutable, writeable_len);
        };

        // Write the data to the pipe.
        let n = min(writeable_len, total_len);
        let mut remaining = n;
        for buf in iovecs.buffers() {
            let chunk_len = min(remaining, buf.len());
            mutable.buffer.extend_from_slice(&buf[..chunk_len]);
            remaining -= chunk_len;
            if remaining == 0 {
                break;
            }
        }

        drop(mutable);

        // Notify the waiters. There might be readers that are waiting
        // for the pipe to be readable.
        self.notify();

        Ok(n)
    }

    fn close(&self) {
        {
            let mut mutable = self.inner.mutable.lock();
            match self.end {
                End::Read => mutable.reader_open = false,
                End::Write => mutable.writer_open = false,
            }
        }

        self.notify();
    }

    fn poll(&self) -> Result<c_short, Errno> {
        let mutable = self.inner.mutable.lock();
        let mut status = 0;
        match self.end {
            End::Read => {
                if !mutable.buffer.is_empty() {
                    status |= POLLIN;
                }

                if !mutable.writer_open {
                    status |= POLLHUP;
                }
            }
            End::Write => {
                if mutable.buffer.len() < PIPE_CAPACITY {
                    status |= POLLOUT;
                }

                if !mutable.reader_open {
                    status |= POLLERR;
                }
            }
        }

        Ok(status)
    }

    fn wait_queue(&self) -> Option<&WaitQueue> {
        Some(&self.inner.wait_queue)
    }
}
