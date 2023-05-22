use core::{
    cell::{Cell, UnsafeCell},
    ops::{Deref, DerefMut},
    panic,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::arch;

/// A `GiantLockGuard` owner tracker for debugging.
///
/// While the giant lock is already held, it does not mean there's only one
/// mutable reference.
///
/// `LockTracker` is to ensure the property and panic if it's violated, just
/// like what `RefCell` does. Maybe we can disable this in release mode but
/// I'll keep it until we're sure that this causes non-negligible overhead.
struct LockTracker {
    lock: AtomicBool,
    locked_at: Cell<Option<&'static panic::Location<'static>>>,
}

impl LockTracker {
    const fn new() -> Self {
        Self {
            lock: AtomicBool::new(false),
            locked_at: Cell::new(None),
        }
    }

    fn acquire(&self, locked_at: &'static panic::Location<'static>) {
        if self
            .lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Failed to acquire the lock. This means that the it's already
            // borrowed.
            panic!(
                "giant lock is already borrowed at {}",
                self.locked_at.take().unwrap()
            );
        }

        self.locked_at.set(Some(locked_at));
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

/// A giant lock.
///
/// The lock will automatically be held by HAL (`arch` module) when it enters
/// the kernel mode. Namely, all `GiantLock` objects will share the same lock
/// and is always held until the CPU returns to the user mode.
///
/// While the CPU keeps the lock, it's possible to have multiple mutable
/// references to the inner value, which is not allowed in Rust. To ensure the
/// property, `LockTracker` will panic if it's violated, just like what `RefCell`
/// does.
pub struct GiantLock<T> {
    inner: UnsafeCell<T>,
    tracker: LockTracker,
}

impl<T> GiantLock<T> {
    /// Creates a new `GiantLock` wrapping the given value.
    pub const fn new(inner: T) -> Self {
        Self {
            inner: UnsafeCell::new(inner),
            tracker: LockTracker::new(),
        }
    }

    /// Returns a mutable reference to the inner value.
    ///
    /// Unlike ordinal locks, **this method never blocks** as the giant lock is
    /// always acquired when the CPU enters the kernel mode.
    ///
    /// This behaves like `RefCell::borrow_mut`: it returns a mutable reference
    /// as a guard object and it'll keep the mutable borrow until the guard is
    /// dropped.
    ///
    /// # Panics
    ///
    /// Panics if:
    ///
    /// - It is already borrowed.
    /// - The giant lock is not held by the current CPU (presumerably a bug in
    ///   `arch` module).
    #[track_caller]
    pub fn borrow_mut(&self) -> GiantLockGuard<'_, T> {
        debug_assert!(arch::owns_giant_lock());

        self.tracker.acquire(panic::Location::caller());

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
