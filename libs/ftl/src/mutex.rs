use ftl_utils::spinlock::SpinLock;
use ftl_utils::spinlock::SpinLockGuard;

use crate::thread::thread_yield;

pub type MutexGuard<'a, T> = SpinLockGuard<'a, T>;

pub struct Mutex<T> {
    inner: SpinLock<T>,
}

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            inner: SpinLock::new(value),
        }
    }

    #[track_caller]
    pub fn lock(&self) -> MutexGuard<'_, T> {
        loop {
            if let Ok(guard) = self.inner.try_lock() {
                return guard;
            }

            thread_yield();
        }
    }
}
