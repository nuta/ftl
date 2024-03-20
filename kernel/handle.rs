use alloc::boxed::Box;
use core::ops::Deref;
use core::ptr::NonNull;
use core::sync::atomic;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;

use ftl_types::handle::HandleId;
use hashbrown::HashMap;

/// A trait for kernel objects that can be referred to by a handle ([`Handle`]).
pub trait Handleable: Sync + Send {}

struct RefCounted<T: Handleable> {
    ref_count: AtomicUsize,
    value: T,
}

/// Handle, a reference-counted pointer to a kernel object with allowed
/// operations on it, aka *"capability"*.
///
/// # Reference Counting
///
/// This type uses some atomic operations to keep track of the number of
/// references to the underlying object. [`Ordering`] parameters are chosen
/// to be as relaxed as possible in the fast path, inspired by Rust's `Arc`
/// implementation.
pub struct Handle<T: Handleable> {
    inner: NonNull<RefCounted<T>>,
}

impl<T: Handleable> Handle<T> {
    /// Creates a new `Handle` to the given value.
    pub fn new(value: T) -> Handle<T> {
        let inner = Box::leak(Box::new(RefCounted {
            ref_count: AtomicUsize::new(1),
            value,
        }));

        Self {
            inner: NonNull::new(inner).unwrap(),
        }
    }

    fn inner_ref(&self) -> &RefCounted<T> {
        // SAFETY: `inner` always points to a valid `RefCounted` object
        //         until you drop the last `Handle` (including this one).
        //
        //         Also, `Handleable` objects are `Sync`, so it's safe
        //         to access them from different threads.
        unsafe { self.inner.as_ref() }
    }
}

impl<T: Handleable> Deref for Handle<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner_ref().value
    }
}

impl<T: Handleable> Clone for Handle<T> {
    fn clone(&self) -> Self {
        let inner = self.inner_ref();

        // Increment the reference count.
        //
        // Theoretically, the counter can overflow, but it's not a problem
        // in practice because having 2^B references (where B is 32 or 64
        // depending on the CPU) means you have at least 2^B * size_of(NonNull)
        // bytes of space. Who would have that much memory to store references
        // to only single object? If you don't agree with this, please open
        // a PR with a nice explanation. It must be fun to read :)
        //
        // That said, if you add a method which increments the reference count
        // without returning a new `Handle`, it can be a problem.
        inner.ref_count.fetch_add(1, Ordering::Relaxed);

        Self { inner: self.inner }
    }
}

impl<T: Handleable> Drop for Handle<T> {
    fn drop(&mut self) {
        let inner = self.inner_ref();

        // Release the reference count.
        if inner.ref_count.fetch_sub(1, Ordering::Release) == 1 {
            atomic::fence(Ordering::Acquire);

            // SAFETY: We are the last `Handle` to this `RefCounted` object,
            //         so we can safely deallocate it.
            unsafe {
                drop(Box::from_raw(self.inner.as_ptr()));
            }
        }
    }
}

unsafe impl<T: Handleable> Sync for Handle<T> {}
unsafe impl<T: Handleable> Send for Handle<T> {}

pub struct HandleTable<T: Handleable> {
    handles: HashMap<HandleId, Handle<T>>,
}
