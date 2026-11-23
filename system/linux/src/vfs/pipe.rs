use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::cmp::min;

use ftl::mutex::Mutex;

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
const PIPE_CAPACITY: usize = 65536;

#[derive(Clone, Copy, PartialEq, Eq)]
enum End {
    Read,
    Write,
}

struct Mutable {
    buffer: VecDeque<u8>,
    reader_open: bool,
    writer_open: bool,
}

struct Inner {
    mutable: Mutex<Mutable>,
    wait_queue: WaitQueue,
}

pub struct Pipe {
    inner: Arc<Inner>,
    end: End,
}

impl Pipe {
    pub fn pair() -> Result<(Self, Self), Errno> {
        let inner = Arc::new(Inner {
            mutable: Mutex::new(Mutable {
                buffer: VecDeque::new(),
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

    fn notify(&self, events: c_short) {
        self.inner.wait_queue.notify(Some(events));
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

        let sleep_guard = sleep.guard(&self.inner.wait_queue);
        loop {
            let mut mutable = self.inner.mutable.lock();
            if !mutable.buffer.is_empty() {
                // Read the data from the pipe.
                let n = min(buf.len(), mutable.buffer.len());
                let (front, back) = mutable.buffer.as_slices();
                let front_len = min(n, front.len());
                buf[..front_len].copy_from_slice(&front[..front_len]);
                buf[front_len..n].copy_from_slice(&back[..n - front_len]);

                // Remove the read data from the pipe.
                mutable.buffer.drain(..n);
                drop(mutable);
                drop(sleep_guard);

                // Notify the waiters. There might be writers that are waiting
                // for the pipe to be writable.
                self.notify(POLLOUT);
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

            if !sleep_guard.is_subscribed() {
                sleep_guard.subscribe()?;
                continue;
            }

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
        let sleep_guard = sleep.guard(&self.inner.wait_queue);
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

                if !sleep_guard.is_subscribed() {
                    sleep_guard.subscribe()?;
                    continue;
                }

                sleep_guard.wait()?;
                continue;
            }

            break (mutable, writeable_len);
        };

        // Write the data to the pipe.
        let n = min(writeable_len, total_len);
        mutable.buffer.try_reserve(n).map_err(|_| Errno::ENOMEM)?;

        let mut remaining = n;
        for buf in iovecs.buffers() {
            let chunk_len = min(remaining, buf.len());
            mutable.buffer.extend(&buf[..chunk_len]);
            remaining -= chunk_len;
            if remaining == 0 {
                break;
            }
        }

        drop(mutable);
        drop(sleep_guard);

        // Notify the waiters. There might be readers that are waiting
        // for the pipe to be readable.
        self.notify(POLLIN);

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

        self.notify(POLLIN | POLLOUT | POLLERR | POLLHUP);
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
