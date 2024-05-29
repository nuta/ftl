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

/// The reference-counted object.
///
/// # Why not `Rc` or `Arc`?
///
/// Rust's standard library provides `Rc` and `Arc` for reference counting.
/// However, they are not suitable for our use case because:
///
/// - We'll never need weak references. Instead, the userland will delete each
///   object explicitly through a system call (lmk if you find a counter-example!).
struct RefCounted<T> {
    counter: AtomicUsize,
    inner: T,
}

/// A reference-counted mutably-borrowable reference.
///
/// This is similar to `Arc<Mutex<T>>`: allows multiple references to the
/// inner value by reference couting, and allows mutable access to the inner
/// value by big kernel lock + runtime borrow checking.
pub struct SharedRef<T> {
    ptr: NonNull<RefCounted<T>>,
}

impl<T> SharedRef<T> {
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

    fn as_ref(&self) -> &RefCounted<T> {
        // SAFETY: We always keep the reference to the object.
        unsafe { self.ptr.as_ref() }
    }

    fn dec_ref(&self) -> bool {
        let counter = self.as_ref().counter.fetch_sub(1, Ordering::Release);

        atomic::fence(Ordering::Acquire);

        counter == 1
    }
}

impl<T> Drop for SharedRef<T> {
    fn drop(&mut self) {
        if self.dec_ref() {
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

unsafe impl<T> Sync for SharedRef<T> {}
unsafe impl<T> Send for SharedRef<T> {}
