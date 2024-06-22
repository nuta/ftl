//! Reference counting.

use alloc::boxed::Box;
use core::any::Any;
use core::marker::Unsize;
use core::mem;
use core::mem::MaybeUninit;
use core::ops::CoerceUnsized;
use core::ops::Deref;
use core::ops::DerefMut;
use core::ptr::NonNull;
use core::sync::atomic;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;

struct RefCounted<T: ?Sized> {
    counter: AtomicUsize,
    value: T,
}

/// Frees the memory of a reference-counted object.
///
/// # Safety
///
/// The caller must ensure that the pointer is valid and it won't be used
/// anymore after calling this function, that is, ensure no double-free
/// or use-after-free.
unsafe fn free_ref_counted<T: ?Sized>(ptr: &NonNull<RefCounted<T>>) {
    let boxed = unsafe { Box::from_raw(ptr.as_ptr()) };
    mem::drop(boxed);
}

pub struct UniqueRef<T: ?Sized> {
    ptr: NonNull<RefCounted<T>>,
}

impl<T> UniqueRef<T> {
    /// Creates a new reference-counted object.
    pub fn new(value: T) -> Self {
        let ptr = Box::leak(Box::new(RefCounted {
            counter: AtomicUsize::new(1),
            value,
        }));

        Self {
            // SAFETY: Box always returns a valid non-null pointer.
            ptr: unsafe { NonNull::new_unchecked(ptr) },
        }
    }

        /// Returns a reference to the inner object.
        fn inner(&self) -> &RefCounted<T> {
            // SAFETY: The object will be kept alive as long as `self` is alive.
            //         The compiler will guarantee `&RefCounted<T>` can't outlive
            //         `self`.
            unsafe { self.ptr.as_ref() }
        }

    /// Returns a mutable reference to the inner object.
    fn inner_mut(&mut self) -> &mut RefCounted<T> {
        // SAFETY: The object will be kept alive as long as `self` is alive.
        //         The compiler will guarantee `&RefCounted<T>` can't outlive
        //         `self`.
        unsafe { self.ptr.as_mut() }
    }
}

impl<T: ?Sized> Drop for UniqueRef<T> {
    fn drop(&mut self) {
        // SAFETY: This object represents a unique reference to the
        //         RefCounted object. No other references exist.
        unsafe {
            free_ref_counted(&self.ptr);
        }
    }
}


impl<T> Deref for UniqueRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner().value
    }
}

impl<T> DerefMut for UniqueRef<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner_mut().value
    }
}


impl<T> Into<SharedRef<T>> for UniqueRef<T> {
    fn into(self) -> SharedRef<T> {
        let sref = SharedRef { ptr: self.ptr };

        // The ownership has been moved into sref. Avoid freeing the
        // RefCounted object.
        mem::forget(self);

        sref
    }
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
///   object explicitly through a system call (LMK if you noticed counter-examples!).
///
/// # Atomic Operations on counters
///
/// [`Ordering`] parameters are chosen to be as relaxed as possible in the fast
/// path, inspired by Rust's `Arc` implementation.
pub struct SharedRef<T: ?Sized> {
    ptr: NonNull<RefCounted<T>>,
}

impl<T> SharedRef<T> {
    /// Creates a new reference-counted object.
    pub fn new(value: T) -> Self {
        UniqueRef::new(value).into()
    }

    pub fn new_uninit() -> SharedRefUninit<T> {

    }
}

impl<T: ?Sized> SharedRef<T> {
    /// Returns a reference to the inner object.
    fn inner(&self) -> &RefCounted<T> {
        // SAFETY: The object will be kept alive as long as `self` is alive.
        //         The compiler will guarantee `&RefCounted<T>` can't outlive
        //         `self`.
        unsafe { self.ptr.as_ref() }
    }
}

impl SharedRef<dyn Any + Sync + Send> {
    pub fn downcast<T>(self) -> Result<SharedRef<T>, Self>
    where
        T: Any + Sync + Send,
    {
        if <dyn Any>::is::<T>(&self) {
            Ok(SharedRef {
                ptr: self.ptr.cast(),
            })
        } else {
            Err(self)
        }
    }
}

impl<T: ?Sized> Drop for SharedRef<T> {
    fn drop(&mut self) {
        // Release the reference count.
        if self.inner().counter.fetch_sub(1, Ordering::Release) == 1 {
            // The reference counter reached zero. Free the memory.

            // "Prevent reordering of use of the data and deletion of the data",
            // as the standard library's `Arc` does [1].
            //
            // [1]: https://github.com/rust-lang/rust/blob/da159eb331b27df528185c616b394bb0e1d2a4bd/library/alloc/src/sync.rs#L2469-L2497
            atomic::fence(Ordering::Acquire);

            // SAFETY: This reference was the last one, so we can safely
            //         free the memory.
            unsafe {
                free_ref_counted(&self.ptr);
            }
        }
    }
}

impl<T> Clone for SharedRef<T> {
    fn clone(&self) -> Self {
        // Increment the reference count.
        //
        // Theoretically, the counter can overflow, but it's not a problem
        // in practice because having 2^B references (where B is 32 or 64
        // depending on the CPU) means you have at least 2^B * size_of(NonNull)
        // bytes of space. Who would have that much memory to store references
        // to only single object?
        //
        // If you don't agree with this, please open a PR with a nice
        // explanation. It must be fun to read :)
        self.inner().counter.fetch_add(1, Ordering::Relaxed);

        Self { ptr: self.ptr }
    }
}

impl<T> Deref for SharedRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner().value
    }
}

unsafe impl<T: Sync> Sync for SharedRef<T> {}
unsafe impl<T: Send> Send for SharedRef<T> {}

// Allow `x as SharedRef<dyn Handleable>`, where `x: SharedRef<T: Handleable>`.
impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<SharedRef<U>> for SharedRef<T> {}
