use core::mem;
use core::mem::size_of;

use ftl::trace;
use ftl_utils::spinlock::SpinLock;

use crate::types::c_short;
use crate::types::errno::Errno;
use crate::types::sys::poll::POLLIN;
use crate::types::sys::poll::POLLOUT;
use crate::vfs::FileLike;
use crate::wait_queue::WaitQueue;

const COUNTER_SIZE: usize = size_of::<u64>();
/// > The maximum value that may be stored in the counter is the largest
/// > unsigned 64-bit value minus 1 (i.e., 0xfffffffffffffffe).
/// >
/// > https://man7.org/linux/man-pages/man2/eventfd.2.html
const COUNTER_MAX_VALUE: u64 = u64::MAX - 1;

pub struct EventFd {
    semaphore: bool,
    counter: SpinLock<u64>,
    wait_queue: WaitQueue,
}

impl EventFd {
    pub fn new(initval: u64, semaphore: bool) -> Result<Self, Errno> {
        Ok(Self {
            semaphore,
            counter: SpinLock::new(initval),
            wait_queue: WaitQueue::new()?,
        })
    }

    fn notify(&self) {
        if let Err(error) = self.wait_queue.notify_all() {
            trace!("failed to notify eventfd waiters: {:?}", error);
        }
    }
}

impl FileLike for EventFd {
    fn read(&self, buf: &mut [u8], _offset: usize, nonblocking: bool) -> Result<usize, Errno> {
        if buf.len() < COUNTER_SIZE {
            return Err(Errno::EINVAL);
        }

        // Wait for the eventfd to be signalled.
        let wq = self.wait_queue.subscribe();
        let mut counter = loop {
            let counter = self.counter.lock();
            if *counter > 0 {
                break counter;
            }

            if nonblocking {
                return Err(Errno::EAGAIN);
            }

            drop(counter);
            wq.wait()?;
        };

        let value = if self.semaphore {
            // Decrement the counter by 1.
            *counter -= 1;
            1
        } else {
            // Read the value, and reset it to 0.
            mem::replace(&mut *counter, 0)
        };

        // Unlock the counter.
        drop(counter);

        // Notify the waiters.
        self.notify();

        // Write the result.
        buf[..size_of::<u64>()].copy_from_slice(&value.to_ne_bytes());
        return Ok(size_of::<u64>());
    }

    fn write(&self, buf: &[u8], _offset: usize, nonblocking: bool) -> Result<usize, Errno> {
        if buf.len() < COUNTER_SIZE {
            return Err(Errno::EINVAL);
        }

        // Read the value.
        let mut tmp = [0u8; COUNTER_SIZE];
        tmp.copy_from_slice(&buf[..COUNTER_SIZE]);
        let value = u64::from_ne_bytes(tmp);
        if value == u64::MAX {
            return Err(Errno::EINVAL);
        }

        // Wait for the eventfd to be writable.
        let wq = self.wait_queue.subscribe();
        let mut counter = loop {
            let counter = self.counter.lock();

            // Block if the counter exceeds the maximum value.
            if counter.saturating_add(value) <= COUNTER_MAX_VALUE {
                break counter;
            }

            if nonblocking {
                return Err(Errno::EAGAIN);
            }

            drop(counter);
            wq.wait()?;
        };

        // Update the counter.
        *counter += value;
        drop(counter);

        // Notify the waiters.
        self.notify();

        return Ok(COUNTER_SIZE);
    }

    fn poll(&self) -> Result<c_short, Errno> {
        let mut status = 0;
        let count = *self.counter.lock();

        if count > 0 {
            status |= POLLIN;
        }

        if count < COUNTER_MAX_VALUE {
            status |= POLLOUT;
        }

        Ok(status)
    }

    fn wait_queue(&self) -> Option<&WaitQueue> {
        Some(&self.wait_queue)
    }
}
