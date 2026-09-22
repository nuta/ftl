use alloc::vec::Vec;
use core::cmp::min;

use ftl::poll::Poll;
use ftl::trace;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_utils::spinlock::SpinLock;

use crate::types::c_short;
use crate::types::errno::Errno;
use crate::types::sys::poll::POLLIN;
use crate::types::sys::poll::POLLOUT;
use crate::vfs::FileLike;
use crate::wait_queue::Sleep;
use crate::wait_queue::WaitQueue;

pub struct Console {
    device: ftl::console::Console,
    pending: SpinLock<Vec<u8>>,
    wait_queue: WaitQueue,
}

impl Console {
    pub fn new() -> Result<Self, Errno> {
        Ok(Self {
            device: ftl::console::Console::open()?,
            pending: SpinLock::new(Vec::new()),
            wait_queue: WaitQueue::new()?,
        })
    }

    pub fn id(&self) -> HandleId {
        self.device.id()
    }

    pub fn subscribe(&self, poll: &Poll) -> Result<(), ErrorCode> {
        self.device.subscribe(poll)
    }

    pub fn handle_rx(&self) {
        let mut tmp = [0; 64];
        loop {
            // TODO: Read into self.pending directly.
            match self.device.read(&mut tmp) {
                Ok(0) | Err(ErrorCode::Empty) => break,
                Ok(n) => {
                    self.pending.lock().extend_from_slice(&tmp[..n]);
                    if let Err(e) = self.wait_queue.notify_all() {
                        trace!("failed to notify poll: {:?}", e);
                    }
                }
                Err(_) => break,
            }
        }
    }
}

impl FileLike for Console {
    fn read(
        &self,
        buf: &mut [u8],
        _offset: usize,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        if buf.is_empty() {
            return Ok(0);
        }

        let sleep_guard = sleep.guard(&self.wait_queue)?;
        let mut pending = loop {
            let pending = self.pending.lock();
            if !pending.is_empty() {
                break pending;
            }

            drop(pending);

            if nonblocking {
                return Err(Errno::EAGAIN);
            }

            if sleep_guard.is_interrupted() {
                return Err(Errno::EINTR);
            }

            sleep_guard.wait()?;
        };

        let n = min(buf.len(), pending.len());
        buf[..n].copy_from_slice(&pending[..n]);
        pending.drain(..n);
        return Ok(n);
    }

    fn write(
        &self,
        buf: &[u8],
        _offset: usize,
        _nonblocking: bool,
        _sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        let mut n = 0;
        while n < buf.len() {
            match ftl::console::write(&buf[n..]) {
                Ok(written) => {
                    n += written;
                }
                Err(_) => {
                    // TODO: Handle error.
                    break;
                }
            }
        }

        Ok(n)
    }

    fn poll(&self) -> Result<c_short, Errno> {
        let mut status = POLLOUT;
        if !self.pending.lock().is_empty() {
            status |= POLLIN;
        }

        Ok(status)
    }

    fn wait_queue(&self) -> Option<&WaitQueue> {
        Some(&self.wait_queue)
    }
}
