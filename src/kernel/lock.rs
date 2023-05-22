use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

use crate::arch;

struct LockTracker {
    lock: AtomicBool,
}

impl LockTracker {
    const fn new() -> Self {
        Self {
            lock: AtomicBool::new(false),
        }
    }

    fn acquire(&self) {
        if self
            .lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Failed to acquire the lock. This means that the it's already
            // borrowed.
            panic!("giant lock is already borrowed");
        }
    }

    fn release(&self) {
        if self
            .lock
            .compare_exchange(true, false, Ordering::Release, Ordering::Relaxed)
            .is_err()
        {
            // Failed to release the lock. This means that the lock is not
            // borrowed.
            panic!("giant lock is not borrowed");
        }
    }
}

pub struct GiantLock<T> {
    inner: UnsafeCell<T>,
    tracker: LockTracker,
}

impl<T> GiantLock<T> {
    pub const fn new(inner: T) -> Self {
        Self {
            inner: UnsafeCell::new(inner),
            tracker: LockTracker::new(),
        }
    }

    pub fn borrow_mut(&self) -> GiantLockGuard<'_, T> {
        debug_assert!(arch::owns_giant_lock());

        self.tracker.acquire();

        GiantLockGuard {
            inner: unsafe { &mut *self.inner.get() },
            tracker: &self.tracker,
        }
    }
}

unsafe impl<T> Sync for GiantLock<T> {}

pub struct GiantLockGuard<'a, T> {
    inner: &'a mut T,
    tracker: &'a LockTracker,
}

impl<'a, T> Deref for GiantLockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner
    }
}

impl<'a, T> DerefMut for GiantLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner
    }
}

impl<'a, T> Drop for GiantLockGuard<'a, T> {
    fn drop(&mut self) {
        self.tracker.release();
    }
}
