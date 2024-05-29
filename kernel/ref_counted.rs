//! Reference counting.
//!
//! # Relationships
//!
//! - `SharedRef<T>`: A reference-counted reference(s). Akin to `Arc<T>` in standard library.

use alloc::boxed::Box;
use core::mem;
use core::ops::Deref;
use core::ptr::NonNull;
use core::sync::atomic;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;

struct RefCounted<T> {
    counter: AtomicUsize,
    inner: T,
}

/// A reference-counted object.
///
/// # Why not `Arc`?
///
/// Rust's standard library provides `Arc` for reference counting. However, we
/// generally prefer rolling our own pritimives in kernel to use what we really
/// need.
///
/// In reference counting, we have some properties:
///
/// - We'll never need weak references. Instead, the userland will delete each
///   object explicitly through a system call (lmk if you find a counter-example!).
pub struct SharedRef<T> {
    ptr: NonNull<RefCounted<T>>,
}

impl<T> SharedRef<T> {
    /// Creates a new reference-counted object.
    pub fn new(inner: T) -> Self {
        let ptr = Box::leak(Box::new(RefCounted {
            counter: AtomicUsize::new(1),
            inner,
        }));

        Self {
            // SAFETY: Box always returns a valid non-null pointer.
            ptr: unsafe { NonNull::new_unchecked(ptr) },
        }
    }

    /// Returns a reference to the inner object.
    fn as_ref(&self) -> &RefCounted<T> {
        // SAFETY: The object will be kept alive as long as `self` is alive.
        //         The compiler will guarantee `&RefCounted<T>` can't outlive
        //         `self`.
        unsafe { self.ptr.as_ref() }
    }

    /// Decrements the reference counter and returns the previous value.
    fn dec_ref(&self) -> usize {
        let counter = self.as_ref().counter.fetch_sub(1, Ordering::Release);
        atomic::fence(Ordering::Acquire);
        counter
    }
}

impl<T> Drop for SharedRef<T> {
    fn drop(&mut self) {
        if self.dec_ref() == 1 {
            // The reference counter reached zero. Free the memory.
            //
            mem::drop(
                // SAFETY: This reference was the last one, so we can safely
                //         free the memory.
                unsafe { Box::from_raw(self.ptr.as_ptr()) },
            );
        }
    }
}

impl<T> Clone for SharedRef<T> {
    fn clone(&self) -> Self {
        self.as_ref().counter.fetch_add(1, Ordering::Relaxed);

        Self { ptr: self.ptr }
    }
}

impl<T> Deref for SharedRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.as_ref().inner
    }
}

unsafe impl<T: Sync> Sync for SharedRef<T> {}
unsafe impl<T: Send> Send for SharedRef<T> {}
