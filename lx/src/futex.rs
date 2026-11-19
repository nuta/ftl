use alloc::sync::Arc;
use alloc::vec::Vec;

use ftl_types::time::MonoTime;
use ftl_utils::spinlock::SpinLock;

use crate::types::errno::Errno;
use crate::types::sys::futex::FUTEX_BITSET_MATCH_ANY;
use crate::wait_queue::Sleep;
use crate::wait_queue::SleepGuard;
use crate::wait_queue::WaitQueue;

struct Waiter {
    uaddr: usize,
    bitset: u32,
    wq: Arc<WaitQueue>,
}

impl Waiter {
    pub fn matches(&self, uaddr: usize, bitset: u32) -> bool {
        self.uaddr == uaddr && self.bitset & bitset != 0
    }
}

pub struct FutexTable {
    waiters: SpinLock<Vec<Waiter>>,
}

impl FutexTable {
    pub fn new() -> Self {
        Self {
            waiters: SpinLock::new(Vec::new()),
        }
    }

    pub fn wait(
        &self,
        uaddr: *const u32,
        val: u32,
        deadline: Option<MonoTime>,
        bitset: u32,
        sleep: Sleep,
    ) -> Result<(), Errno> {
        let wq = Arc::new(WaitQueue::new()?);
        let guard = sleep.guard(&wq);

        let mut waiters = self.waiters.lock();

        // Check the current value.
        //
        // FIXME: Page fault if the address is not mapped, and keeps the
        //        lock held.
        let current = unsafe { uaddr.read_volatile() };
        if current != val {
            return Err(Errno::EAGAIN);
        }

        // Need to block. Enqueue the waiter.
        waiters.push(Waiter {
            uaddr: uaddr.addr(),
            bitset,
            wq: wq.clone(),
        });
        drop(waiters);

        loop {
            let result = if guard.is_subscribed() {
                // Block until woken up by wake/timeout/signal. Do not use "?"
                // here to remove waiter regardless of the result.
                sleep_on_wq(&guard, deadline)
            } else {
                // Subscribe to the wait queue, and continue to check if this
                // thread has already been woken up.
                guard.subscribe().map_err(Errno::from)
            };

            // Check if this thread has been woken up. If so, the our waiter
            // object does not exist in the queue anymore.
            let mut waiters = self.waiters.lock();
            let Some(index) = waiters.iter().position(|w| Arc::ptr_eq(&w.wq, &wq)) else {
                // The waiter is missing. This means it was removed by wake.
                return Ok(());
            };

            if let Err(errno) = result {
                // Woken up by timeout or signal.
                waiters.remove(index);
                return Err(errno);
            }
        }
    }

    pub fn wake(&self, uaddr: usize, count: usize, bitset: u32) -> Result<usize, Errno> {
        // Find the waiters to wake up, up to `count`.
        let to_wake: Vec<Waiter> = self
            .waiters
            .lock()
            .extract_if(.., |w| w.matches(uaddr, bitset))
            .take(count)
            .collect();

        for waiter in &to_wake {
            waiter.wq.notify(None);
        }

        Ok(to_wake.len())
    }

    pub fn requeue(
        &self,
        uaddr: *const u32,
        uaddr2: usize,
        wake_count: usize,
        requeue_count: usize,
        expected: Option<u32>,
    ) -> Result<usize, Errno> {
        let mut waiters = self.waiters.lock();

        // Check the current value while holding the lock.
        if let Some(expected) = expected {
            let current = unsafe { uaddr.read_volatile() };
            if current != expected {
                return Err(Errno::EAGAIN);
            }
        }

        // Remove up to wake_count waiters to wake them up.
        let to_wake: Vec<Waiter> = waiters
            .extract_if(.., |w| w.matches(uaddr.addr(), FUTEX_BITSET_MATCH_ANY))
            .take(wake_count)
            .collect();

        // List up to requeue_count of the remaining waiters.
        let to_requeue = waiters
            .iter_mut()
            .filter(|w| w.matches(uaddr.addr(), FUTEX_BITSET_MATCH_ANY))
            .take(requeue_count);

        // Move them to `uaddr2`.
        let mut requeued = 0;
        for waiter in to_requeue {
            waiter.uaddr = uaddr2;
            requeued += 1;
        }

        // Release the lock before waking up the waiters.
        drop(waiters);

        // Wake up the waiters.
        for waiter in &to_wake {
            waiter.wq.notify(None);
        }

        Ok(to_wake.len() + requeued)
    }
}

fn sleep_on_wq(guard: &SleepGuard<'_>, deadline: Option<MonoTime>) -> Result<(), Errno> {
    if guard.is_interrupted() {
        return Err(Errno::EINTR);
    }

    match deadline {
        Some(deadline) => {
            if guard.wait_until(deadline)? {
                return Err(Errno::ETIMEDOUT);
            }
        }
        None => guard.wait()?,
    }

    Ok(())
}
