use core::{ops::Deref, ptr::{NonNull, drop_in_place}, sync::atomic::AtomicUsize};

use crate::giant_lock::{GiantLock, GiantLockGuard};

/// A trait for types that contain a reference counter in the intrusive way.
///
/// # Why not `Rc` or `Arc`?
///
/// Rust's standard library provides [`Rc`] and [`Arc`] for reference counting.
/// However, they are not suitable for our use case because:
///
/// - For clarity, we want to make the memory layout of a reference-counted
///   object explicit, i.e. make it intrusive!
/// - We'll never need weak references. Instead, the userland will delete each
///   object explicitly through a system call (lmk if you find a counter-example!).
/// - While we don't use weak references, we still want to distinguish between
///   always-alive and may-be-dead references: the pointer from a thread to its
///   process should be always-alive, while the pointer from a communication channel
///   to its destination process may be dead (e.g. `EPIPE` in UNIX).
/// - The implementation of reference counting depends on how we lock the kernel:
///   if we just use a global lock [`GiantLock`] we don't need atomic operations
///   as the lock guarantees that only one thread can access the reference counter
///   at a time.
struct RefCounted<T> {
    counter: usize,
    inner: T,
}

impl<T> RefCounted<T> {
    const fn new(inner: T) -> RefCounted<T> {
        RefCounted { counter: 1, inner }
    }

    // Increments the reference counter.
    fn inc_ref(&mut self) {
        self.counter += 1;

        // TODO: Should we handle overflow?
    }

    /// Decrements the reference counter and returns `true` if the counter reaches
    /// zero, i.e. the caller should drop the object manually.
    fn dec_ref(&mut self) -> bool {
        debug_assert!(self.counter > 0);

        self.counter -= 1;
        self.counter == 0
    }
}

pub struct LockedRef<T> {
    ptr: NonNull<GiantLock<RefCounted<T>>>,
}

impl<T> LockedRef<T> {
    pub const fn new(ptr: NonNull<GiantLock<RefCounted<T>>>) -> LockedRef<T> {
        LockedRef { ptr }
    }

    pub fn borrow_mut(&self) -> GiantLockGuard<'_, RefCounted<T>> {
        (unsafe { self.ptr.as_ref() }).borrow_mut()
    }

    pub fn inc_ref(&self) -> LockedRef<T> {
        self.borrow_mut().inc_ref();
        LockedRef { ptr: self.ptr }
    }
}

impl<T> Drop for LockedRef<T> {
    fn drop(&mut self) {
        if self.borrow_mut().dec_ref() {
            // The reference counter reached zero. Drop the inner value
            // and free the memory.

            // Safety: This reference was the last one, so we can safely
            //         mutate the inner value and drop it.
            unsafe {
                // Call the destructor of the inner value (i.e. Drop).
                drop_in_place(self.ptr.as_mut());

                // Mark the memory as free (untyped).
                // TODO:
            }
        }
    }
}
