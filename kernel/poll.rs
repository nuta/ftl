use alloc::vec::Vec;

use crate::arch;
use crate::cpuvar::current_thread;
use crate::ref_counted::SharedRef;
use crate::spinlock::SpinLock;
use crate::spinlock::SpinLockGuard;
use crate::thread::Thread;

struct Poller {
    thread: SharedRef<Thread>,
}

pub enum Readiness<T> {
    Ready(T),
    Sleep,
}

pub struct PollPoint {
    pollers: SpinLock<Vec<Poller>>,
}

impl PollPoint {
    pub fn wake(&self) {
        for poller in self.pollers.lock().drain(..) {
            poller.thread.resume();
        }
    }

    pub fn may_block<'a, F, T, U>(&self, lock: &'a SpinLock<T>, is_ready: F) -> U
    where
        F: Fn(&mut SpinLockGuard<'a, T>) -> Readiness<U>,
    {
        loop {
            let mut pollers = self.pollers.lock();
            let mut guard = lock.lock();
            match is_ready(&mut guard) {
                Readiness::Ready(ret) => {
                    return ret;
                }
                Readiness::Sleep => {
                    pollers.push(Poller {
                        thread: current_thread().clone(),
                    });

                    // Release the lock.
                    drop(guard);
                    drop(pollers);

                    arch::yield_cpu();
                }
            }
        }
    }
}
