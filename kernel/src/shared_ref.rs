//! Reference counting.
use alloc::alloc::Layout;
use alloc::alloc::alloc;
use alloc::boxed::Box;
use core::fmt;
use core::marker::Unsize;
use core::mem;
use core::mem::ManuallyDrop;
use core::mem::offset_of;
use core::ops::CoerceUnsized;
use core::ops::Deref;
use core::ptr::NonNull;
use core::sync::atomic;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;

use ftl_api::error::ErrorCode;
use ftl_api::handle::Handle;
use ftl_api::handle::HandleRight;

/// The storage for a reference-counted object.
///
/// `SharedRef<T>`s store a pointer to this struct.
pub struct RefCounted<T: ?Sized> {
    counter: AtomicUsize,
    value: T,
}

impl<T> RefCounted<T> {
    const fn new(value: T) -> Self {
        Self {
            counter: AtomicUsize::new(1),
            value,
        }
    }

    pub const fn new_static(value: T) -> Self {
        Self {
            // TODO: Better way to guarantee the static reference won't be dropped.
            counter: AtomicUsize::new(usize::MAX / 2),
            value,
        }
    }
}

/// A reference-counted object.
///
/// # Why not `Arc`?
///
/// Rust's standard library provides `Arc` for reference counting. However, we
/// generally prefer rolling our own primitives in kernel to use what we really
/// need:
///
/// - Allocation errors must be handled by the caller, not panicking.
///
/// - Some objects are statically allocated, and can be initialized at compile
///   time. We want to avoid Lazy<SharedRef<T>>, which has an overhead of
///   the check for emptiness.
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
    pub fn new(value: T) -> Result<Self, ErrorCode> {
        let layout = Layout::new::<RefCounted<T>>();
        let ptr = unsafe { alloc(layout) as *mut RefCounted<T> };
        if ptr.is_null() {
            return Err(ErrorCode::OUT_OF_MEMORY);
        }

        unsafe {
            ptr.write(RefCounted::new(value));
        }

        Ok(Self {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
        })
    }

    /// Creates a new reference-counted object from a static reference.
    pub const fn new_static(inner: &'static RefCounted<T>) -> Self {
        let ptr = inner as *const RefCounted<T> as *mut RefCounted<T>;
        Self {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
        }
    }

    /// Provides a raw pointer to the data.
    ///
    /// The counts are not affected in any way and the SharedRef is not consumed.
    /// The pointer is valid for as long as there are strong counts in the SharedRef.
    pub fn as_ptr(&self) -> *const T {
        &self.inner().value as *const T
    }

    /// Returns true if the two `SharedRef`s point to the same object.
    pub fn eq(a: &SharedRef<T>, b: &SharedRef<T>) -> bool {
        core::ptr::eq(a.ptr.as_ptr(), b.ptr.as_ptr())
    }

    /// Consumes the SharedRef, returning the wrapped pointer.
    ///
    /// To avoid a memory leak the pointer must be converted back to a SharedRef
    /// using `SharedRef::from_raw`.
    pub fn into_raw(self) -> *const T {
        let ptr = self.as_ptr();
        core::mem::forget(self);
        ptr
    }

    /// Constructs a SharedRef from a raw pointer. The reference count
    /// is not incremented.
    ///
    /// # Safety
    ///
    /// The raw pointer must have been previously returned by a call to
    /// `SharedRef::into_raw`.
    pub unsafe fn from_raw(ptr: *const T) -> Self {
        unsafe {
            // Calculate the pointer to RefCounted<T> from the pointer to T
            let ref_counted =
                (ptr as *const u8).sub(offset_of!(RefCounted<T>, value)) as *mut RefCounted<T>;
            Self {
                ptr: NonNull::new_unchecked(ref_counted),
            }
        }
    }

    /// Clones a reference-counted object from a raw pointer, created by
    /// `SharedRef::into_raw`.
    ///
    /// # Safety
    ///
    /// The raw pointer must have been previously returned by a call to
    /// `SharedRef::into_raw`.
    pub unsafe fn clone_from_raw(ptr: *const T) -> Self {
        // SAFETY: The caller must ensure SharedRef::from_raw's safety
        //         conditions.
        let borrowed = ManuallyDrop::new(unsafe { Self::from_raw(ptr) });

        // Self::from_raw does not increment the reference count. Do it manually,
        // and avoid triggering the destructor (decrement the count) by using
        // ManuallyDrop.
        SharedRef::clone(&borrowed)
    }

    /// Unwraps a borrowed handle into a reference to the kernel object.
    pub fn from_borrowed_handle(
        handle: &Handle,
        action: HandleRight,
    ) -> Result<SharedRef<T>, ErrorCode>
    where
        T: 'static,
    {
        // TODO: Is it safe to skip this check, assuming that ftl_api
        // guarantees that the handle has the correct type?
        if !handle.is_type::<T>() {
            return Err(ErrorCode::INVALID_TYPE);
        }

        if !handle.authorize(action) {
            return Err(ErrorCode::NOT_ALLOWED);
        }

        // SAFETY: &Handle ensures that the object is still alive,
        //         and we checked the handle has the correct type.
        Ok(unsafe { Self::clone_from_raw(handle.raw() as *const T) })
    }

    /// Unwraps an owned handle into a reference to the kernel object.
    pub fn from_moved_handle(handle: Handle) -> Result<SharedRef<T>, ErrorCode>
    where
        T: 'static,
    {
        if !handle.is_type::<T>() {
            // TODO: Free the shared reference to the object.
            //
            // TODO: Is it safe to skip this check, assuming that ftl_api
            // guarantees that the handle has the correct type?
            return Err(ErrorCode::INVALID_TYPE);
        }

        let ptr = handle.raw() as *const T;
        Ok(unsafe { Self::from_raw(ptr) })
    }
}

/// A kernel object that can be exposed to servers as a [`Handle`].
pub trait Handleable: Sized + 'static {
    const DEFAULT_RIGHT: HandleRight;
}

impl<T: Handleable> SharedRef<T> {
    pub fn into_handle(self) -> Handle {
        let raw = self.into_raw() as usize;
        Handle::new::<T>(raw, T::DEFAULT_RIGHT)
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

impl<T: ?Sized> Drop for SharedRef<T> {
    fn drop(&mut self) {
        debug_assert!(self.inner().counter.load(Ordering::Relaxed) > 0);

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
            mem::drop(unsafe { Box::from_raw(self.ptr.as_ptr()) });
        }
    }
}

impl<T: ?Sized> Clone for SharedRef<T> {
    fn clone(&self) -> Self {
        debug_assert!(self.inner().counter.load(Ordering::Relaxed) > 0);

        // Increment the reference count.
        //
        // Theoretically, the counter can overflow, but it's not a problem
        // in practice because having 2^B references (where B is 32 or 64
        // depending on the CPU) means you have at least 2^B * size_of(NonNull)
        // bytes of space. Who would have that much memory to store references
        // to a *single* object?
        self.inner().counter.fetch_add(1, Ordering::Relaxed);

        Self { ptr: self.ptr }
    }
}

impl<T: ?Sized> Deref for SharedRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner().value
    }
}

impl<T> fmt::Debug for SharedRef<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SharedRef")
            .field(&self.inner().value)
            .finish()
    }
}

unsafe impl<T: Sync + Send + ?Sized> Sync for SharedRef<T> {}
unsafe impl<T: Sync + Send + ?Sized> Send for SharedRef<T> {}

impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<SharedRef<U>> for SharedRef<T> {}
