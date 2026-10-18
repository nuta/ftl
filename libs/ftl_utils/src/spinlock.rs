//! Spinlock implementation.
use core::cell::UnsafeCell;
use core::ops::Deref;
use core::ops::DerefMut;
use core::ptr;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering;

pub struct TryLockError;

/// A simple spinlock.
pub struct SpinLock<T: ?Sized> {
    lock: AtomicBool,
    value: UnsafeCell<T>,
}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> SpinLock<T> {
        SpinLock {
            value: UnsafeCell::new(value),
            lock: AtomicBool::new(false),
        }
    }

    #[track_caller]
    pub fn try_lock(&self) -> Result<SpinLockGuard<'_, T>, TryLockError> {
        if self
            .lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return Err(TryLockError);
        }

        Ok(SpinLockGuard { this: self })
    }

    #[track_caller]
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        loop {
            if let Ok(guard) = self.try_lock() {
                return guard;
            }

            core::hint::spin_loop();
        }
    }

    /// Locks two spinlocks in a consistent order to avoid deadlock.
    #[track_caller]
    pub fn lock_two<'a>(
        a: &'a SpinLock<T>,
        b: &'a SpinLock<T>,
    ) -> (SpinLockGuard<'a, T>, SpinLockGuard<'a, T>) {
        assert!(!core::ptr::eq(a, b));

        if ptr::from_ref(a).addr() < ptr::from_ref(b).addr() {
            let a_guard = a.lock();
            let b_guard = b.lock();
            (a_guard, b_guard)
        } else {
            let b_guard = b.lock();
            let a_guard = a.lock();
            (a_guard, b_guard)
        }
    }
}

pub struct SpinLockGuard<'a, T: ?Sized + 'a> {
    this: &'a SpinLock<T>,
}

impl<T: ?Sized> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.this.lock.store(false, Ordering::Release);
    }
}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: The dereference is safe because this lock guard has
        // exclusive access to the data and the pointer is always valid.
        unsafe { &*self.this.value.get() }
    }
}

impl<T> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: The dereference is safe because this lock guard has
        // exclusive access to the data and the pointer is always valid.
        unsafe { &mut *self.this.value.get() }
    }
}

unsafe impl<T: ?Sized + Send> Sync for SpinLock<T> {}
unsafe impl<T: ?Sized + Send> Send for SpinLock<T> {}

unsafe impl<T: ?Sized + Sync> Sync for SpinLockGuard<'_, T> {}
unsafe impl<T: ?Sized + Send> Send for SpinLockGuard<'_, T> {}
