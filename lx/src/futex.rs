use alloc::sync::Arc;
use alloc::vec::Vec;

use ftl_types::time::MonoTime;
use ftl_utils::spinlock::SpinLock;

use crate::types::errno::Errno;
use crate::wait_queue::Sleep;
use crate::wait_queue::SleepGuard;
use crate::wait_queue::WaitQueue;

struct Waiter {
    uaddr: usize,
    bitset: u32,
    wq: WaitQueue,
}

impl Waiter {
    pub fn new(uaddr: usize, bitset: u32) -> Result<Self, Errno> {
        let wq = WaitQueue::new()?;
        Ok(Self { uaddr, bitset, wq })
    }

    pub fn matches(&self, uaddr: usize, bitset: u32) -> bool {
        self.uaddr == uaddr && self.bitset & bitset != 0
    }
}

pub struct FutexTable {
    waiters: SpinLock<Vec<Arc<Waiter>>>,
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
        let waiter = Arc::new(Waiter::new(uaddr.addr(), bitset)?);
        let guard = sleep.guard(&waiter.wq)?;

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
        waiters.push(waiter.clone());
        drop(waiters);

        // Block until woken up by wake/timeout/signal. Do not use "?" here to
        // remove waiter regardless of the result.
        let result = sleep_on_wq(&guard, deadline);

        // Woken up by wake/timeout/signal. Remove our waiter.
        let mut waiters = self.waiters.lock();
        let Some(index) = waiters.iter().position(|w| Arc::ptr_eq(w, &waiter)) else {
            // The waiter is missing. This means it was removed by wake.
            return Ok(());
        };

        // Woken up by timeout or signal.
        waiters.remove(index);

        if let Err(errno) = result {
            // If sleep_on_wq knows why we woke up, return that.
            return Err(errno);
        }

        // TODO: Does this happen?
        Err(Errno::EINTR)
    }

    pub fn wake(&self, uaddr: usize, count: usize, bitset: u32) -> Result<usize, Errno> {
        // Find the waiters to wake up, up to `count`.
        let to_wake: Vec<Arc<Waiter>> = self
            .waiters
            .lock()
            .extract_if(.., |w| w.matches(uaddr, bitset))
            .take(count)
            .collect();

        for waiter in &to_wake {
            waiter.wq.notify_all()?;
        }

        Ok(to_wake.len())
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
