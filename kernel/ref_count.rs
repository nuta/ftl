use core::{
    mem::{self, align_of, size_of, MaybeUninit},
    ops::{Deref, DerefMut},
    ptr::{drop_in_place, NonNull},
};

use essentials::{alignment::align_up, static_assert};

use crate::{
    address::PAddr,
    arch::PAGE_SIZE,
    giant_lock::{GiantLock, GiantLockGuard},
};

/// The reference counter.
///
/// # Why not `Rc` or `Arc`?
///
/// Rust's standard library provides [`Rc`] and [`Arc`] for reference counting.
/// However, they are not suitable for our use case because:
///
/// - The implementation of reference counting depends on how we lock the kernel:
///   if we just use a global lock [`GiantLock`] we don't need atomic operations
///   as the lock guarantees that only one thread can access the reference counter
///   at a time.
/// - We'll never need weak references. Instead, the userland will delete each
///   object explicitly through a system call (lmk if you find a counter-example!).
struct RefCounted<T> {
    counter: usize,
    inner: T,
}

impl<T> RefCounted<T> {
    /// Creates a reference counted object.
    const fn new(inner: T) -> RefCounted<T> {
        RefCounted { counter: 0, inner }
    }

    /// Increments the reference counter.
    ///
    /// # Safety
    ///
    /// The caller must ensure tracking the reference and decrementing the
    /// reference counter when dropping the reference.
    unsafe fn inc_ref(&mut self) {
        self.counter += 1;

        // TODO: Should we handle overflow?
    }

    /// Decrements the reference counter and returns `true` if the counter reaches
    /// zero, i.e. the caller should drop the object manually.
    ///
    /// # Safety
    ///
    /// The caller must ensure it corresponds to a previous call to [`inc_ref`].
    fn dec_ref(&mut self) -> bool {
        debug_assert!(self.counter > 0);

        self.counter -= 1;
        self.counter == 0
    }

    /// Returns a mutable reference to the inner value.
    ///
    /// # Safety
    ///
    /// The caller must ensure no other reference to the inner value exists,
    /// e.g. by locking the kernel.
    unsafe fn as_mut(&mut self) -> *mut T {
        &mut self.inner
    }
}

const fn required_num_pages<T>() -> usize {
    align_up(size_of::<T>(), PAGE_SIZE) / PAGE_SIZE
}

/// The type of the value a [`SharedRef`] points to: a pointer to the inner
/// value (`NonNull<T>`) with a reference counter (`RefCounted`) and runtime
/// borrow checker (`GiantLock`).
///
/// You may find this type definition weird: normally we'd use `Arc<Mutex<T>>`
/// in Rust, however we wraps the reference counter with a lock (i.e. akin to
/// `Mutex<Arc<T>>`). Of course this is intentional:
///
/// - We don't need atomic operations to manipulate the reference counter: `GiantLock`
///   guarantees that only one thread can access the counter at a time.
/// - In release builds, [`GiantLock::borrow_mut`] is a no-op (in the future!), so
///   there's no runtime overhead. We just increment/decrement the counter.
///
/// In short, consider this as `ArcMutex<T>`, i.e. integrating `Arc` and `Mutex`
/// deeply into a single useful object ([`SharedRef<T>`]).
#[repr(transparent)]
pub struct SharedRefInner<T> {
    inner: GiantLock<RefCounted<NonNull<T>>>,
}

// Make sure `SharedRefInner<T>`'s layout is the same regardless of `T`.
static_assert!(
    size_of::<SharedRefInner::<()>>()
        == size_of::<SharedRefInner::<[u8; 512]>>()
);
static_assert!(
    align_of::<SharedRefInner::<()>>()
        == align_of::<SharedRefInner::<[u8; 512]>>()
);

impl<T> SharedRefInner<T> {
    /// Creates a new reference-counted object.
    ///
    /// # Safety
    ///
    /// The caller must create at least one reference to the inner value
    /// right after calling this function. The initial reference count is
    /// zero and it's UB to drop the object with zero references.
    pub unsafe fn new(value: NonNull<T>) -> SharedRefInner<T> {
        SharedRefInner {
            inner: GiantLock::new(RefCounted::new(value)),
        }
    }
}

/// A reference-counted mutably-borrowable reference.
///
/// This is similar to [`Arc<Mutex<T>>`]: allows multiple references to the
/// inner value by reference couting, and allows mutable access to the inner
/// value by big kernel lock + runtime borrow checking.
pub struct SharedRef<T> {
    ptr: NonNull<SharedRefInner<T>>,
}

impl<T> SharedRef<T> {
    pub fn new(inner: &mut SharedRefInner<T>) -> SharedRef<T> {
        // Safety: `inner` is a valid pointer.
        let ptr = unsafe { NonNull::new_unchecked(inner as *mut _) };
        let sref = SharedRef { ptr };

        // Safety: The destructor of `sref` will decrement the reference
        //         counter.
        unsafe {
            sref.borrow_inner_mut().inc_ref();
        }

        sref
    }

    fn borrow_inner_mut(&self) -> GiantLockGuard<'_, RefCounted<NonNull<T>>> {
        (unsafe { self.ptr.as_ref() }).inner.borrow_mut()
    }

    pub fn paddr(&self) -> PAddr {
        todo!()
    }

    pub unsafe fn from_paddr(paddr: PAddr) -> Option<SharedRef<T>> {
        todo!()
    }

    /// Returns the mutable reference.
    ///
    /// **Warning:** This method may panic. See [`GiantLock::borrow_mut`]
    /// for more details.
    pub fn borrow_mut(&self) -> GiantLockGuard<'_, T> {
        self.borrow_inner_mut().map(|ref_counted| {
            // Safety: GiantLockGuard ensures that only one thread can access
            //         the inner value at a time.
            unsafe { ref_counted.inner.as_mut() }
        })
    }

    /// Duplicates the reference.
    pub fn inc_ref(this: &SharedRef<T>) -> SharedRef<T> {
        // Safety: The destructor of `SharedRef` will decrement the reference
        //         counter.
        unsafe {
            this.borrow_inner_mut().inc_ref();
        }

        SharedRef { ptr: this.ptr }
    }

    pub unsafe fn dec_ref(this: &SharedRef<T>) {
        // Safety: The caller must ensure that it will not use the reference
        //         after calling this method.
        this.borrow_inner_mut().dec_ref();
    }

    pub unsafe fn leak(this: SharedRef<T>) {
        // Safety: The caller must ensure that it will
        mem::forget(this);
    }
}

impl<T> Drop for SharedRef<T> {
    fn drop(&mut self) {
        if self.borrow_inner_mut().dec_ref() {
            // The reference counter reached zero. Drop the inner value
            // and free the memory.

            // Safety: This reference was the last one, so we can safely
            //         mutate the inner value and drop it.
            unsafe {
                // Call the destructor of the inner value (i.e. Drop).
                drop_in_place(self.ptr.as_mut());

                // Mark the memory frames as unused.
                // FIXME:
                // let vaddr = VAddr::new(self.ptr.as_ptr() as usize);
                // let num_pages = required_num_pages::<SharedRefInner<T>>();
                // retype_frames_as_unused(vaddr, num_pages);
            }
        }
    }
}

/// A unique reference.
///
/// This is similar to [`Box<T>`]: it owns the inner value and drops it when
/// dropped.
pub struct UniqueRef<T> {
    ptr: NonNull<T>,
}

impl<T> UniqueRef<T> {
    /// Creates a new unique reference.
    ///
    /// Returns `None`
    pub fn new(sref: SharedRef<T>) -> Option<UniqueRef<T>> {
        let sref = sref.borrow_inner_mut();
        if sref.counter != 1 {
            // There are other references.
            return None;
        }

        Some(UniqueRef { ptr: sref.inner })
    }
}

impl<T> Deref for UniqueRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // Safety: The compiler guarantees the pointer is still alive and no
        //         mutable reference exists.
        unsafe { self.ptr.as_ref() }
    }
}

impl<T> DerefMut for UniqueRef<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: The compiler guarantees the pointer is still alive and no
        //         other references exist because this method requires `&mut self`.
        unsafe { self.ptr.as_mut() }
    }
}

impl<T> Drop for UniqueRef<T> {
    fn drop(&mut self) {
        unsafe {
            // Mark the memory frames as unused.
            // TODO:
        }
    }
}
