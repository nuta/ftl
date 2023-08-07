use core::{
    cell::{Cell, UnsafeCell},
    mem,
    ops::{Deref, DerefMut},
    panic,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{arch, backtrace::CapturedBacktrace};

/// A mutable reference tracker.
///
/// While the giant lock prevents concurrent access to the inner value,
/// it does not mean there's only one mutable reference.
///
/// `LockTracker` is to ensure the property by panicking if it's violated,
/// just like what `RefCell` does.
///
/// TODO: I plan to disable this in release build to eliminate the overhead.
struct LockTracker {
    lock: AtomicBool,
    locked_at: Cell<Option<CapturedBacktrace>>,
}

impl LockTracker {
    const fn new() -> Self {
        Self {
            lock: AtomicBool::new(false),
            locked_at: Cell::new(None),
        }
    }

    fn acquire(&self, locked_at: CapturedBacktrace) {
        if self
            .lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Failed to acquire the lock. This means that the it's already
            // borrowed and it indicates a bug in the kernel (multiple mutable
            // references).
            panic!(
                "already borrowed at:\n{:?}",
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
            // borrowed and should never happen.
            panic!("giant lock is not borrowed");
        }

        self.locked_at.set(None);
    }
}

/// A giant lock. TL;DR: it's `RefCell` but with `Sync` (shareable between
/// multiple CPUs).
///
/// All `GiantLock` objects will share the same lock, aka "Big Kernel Lock".
/// The lock will automatically be held in `arch` module when it enters
/// the kernel mode and held until the CPU returns to the user mode.
///
/// To prevent multiple mutable references to the inner value,
/// which is not allowed in Rust, [`GiantLock::borrow_mut`] will panics
/// just like `RefCell::borrow_mut`.
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
    pub fn borrow_mut(&self) -> GiantLockGuard<'_, T> {
        debug_assert!(arch::owns_giant_lock());

        self.tracker.acquire(CapturedBacktrace::capture());

        GiantLockGuard {
            inner: unsafe { self.inner.get() },
            tracker: &self.tracker,
        }
    }
}

// SAFETY: The giant lock ensures that the inner value will be accessible at
//         most one CPU (or thread) at a time.
unsafe impl<T> Sync for GiantLock<T> {}

/// A mutable reference to the inner value of [`GiantLock`].
///
/// Only one `GiantLockGuard` to a same `GiantLock` can exist at a time and
/// the borrow will automatically be released when the it is dropped.
#[clippy::has_significant_drop]
pub struct GiantLockGuard<'a, T> {
    inner: *mut T,
    tracker: &'a LockTracker,
}

impl<'a, T> GiantLockGuard<'a, T> {
    pub fn map<U, F>(
        mut guard: GiantLockGuard<'a, T>,
        f: F,
    ) -> GiantLockGuard<'a, U>
    where
        F: FnOnce(&mut T) -> &mut U,
    {
        // SAFETY: Holding a GiantLockGuard means that the giant lock is
        //         held and the runtime borrow checker checked that there's
        //         no other mutable reference to the inner value.
        let inner = f(unsafe { &mut *guard.inner });
        let tracker = guard.tracker;

        // Create a new guard object and forget the old one, i.e. don't
        // call the destructor for the old one.
        //
        // The destructor will behave in the same way for U as it does for T. In
        // both cases, the tracker will mark the lock as not borrowed.
        mem::forget(guard);
        GiantLockGuard { inner, tracker }
    }
}

impl<'a, T> Deref for GiantLockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: Holding a GiantLockGuard means that the giant lock is
        //         held and the runtime borrow checker checked that there's
        //         no other mutable reference to the inner value.
        unsafe { &*self.inner }
    }
}

impl<'a, T> DerefMut for GiantLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: Holding a GiantLockGuard means that the giant lock is
        //         held and the runtime borrow checker checked that there's
        //         no other mutable reference to the inner value.
        unsafe { &mut *self.inner }
    }
}

impl<'a, T> Drop for GiantLockGuard<'a, T> {
    fn drop(&mut self) {
        self.tracker.release();
    }
}
