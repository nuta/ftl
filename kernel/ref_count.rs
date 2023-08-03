use core::{
    mem::{self, align_of, size_of, MaybeUninit},
    ops::{Deref, DerefMut},
    ptr::{drop_in_place, NonNull},
};

use essentials::{alignment::align_up, static_assert};

use crate::{
    address::{PAddr, VAddr},
    arch::PAGE_SIZE,
    giant_lock::{GiantLock, GiantLockGuard},
    memory_pool::memory_pool_mut,
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
    object: T,
}

impl<T> RefCounted<T> {
    /// Creates a reference counted object.
    const fn new(object: T) -> RefCounted<T> {
        RefCounted { counter: 0, object }
    }

    /// Increments the reference counter.
    ///
    /// # Safety
    ///
    /// The caller must ensure tracking the reference and decrementing the
    /// reference counter when dropping the reference.
    unsafe fn inc_ref(&mut self) {
        // TODO: Should we handle overflow?
        self.counter += 1;
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
        &mut self.object
    }
}

/// The type of the value a [`SharedRef`] points to: a pointer to the inner
/// value (`NonNull<T>`) with a reference counter (`RefCounted`) and runtime
/// borrow checker (`GiantLock`).
///
/// You may find this type definition weird: normally we'd use `Arc<Mutex<T>>`
/// in Rust, however we wraps the reference counter with a lock (i.e. akin to
/// `Mutex<Arc<T>>`). Of course this is intentional:
///
/// - We don't need atomic operations to manipulate the reference counter:
///   `GiantLock` guarantees that only one thread can access the counter at a
///   time.
///
/// - In release builds, [`GiantLock::borrow_mut`] is a no-op (in the future!),
///   so there's no runtime overhead. We just increment/decrement the counter.
///
/// In short, consider this as `ArcMutex<T>`, i.e. integrating `Arc` and `Mutex`
/// deeply into a single useful object ([`SharedRef<T>`]).
#[repr(transparent)]
pub struct SharedObject<T> {
    lock: GiantLock<RefCounted<NonNull<T>>>,
}

// Make sure `SharedObject<T>`'s layout is the same regardless of `T`.
static_assert!(
    size_of::<SharedObject::<()>>() == size_of::<SharedObject::<[u8; 512]>>()
);
static_assert!(
    align_of::<SharedObject::<()>>() == align_of::<SharedObject::<[u8; 512]>>()
);

impl<T> SharedObject<T> {
    /// Creates a new reference-counted object.
    ///
    /// # Safety
    ///
    /// The caller must create at least one reference to the inner value
    /// right after calling this function. The initial reference count is
    /// zero and it's UB to drop the object with zero references.
    pub unsafe fn new(value: NonNull<T>) -> SharedObject<T> {
        SharedObject {
            lock: GiantLock::new(RefCounted::new(value)),
        }
    }
}

/// A reference-counted mutably-borrowable reference.
///
/// This is similar to [`Arc<Mutex<T>>`]: allows multiple references to the
/// inner value by reference couting, and allows mutable access to the inner
/// value by big kernel lock + runtime borrow checking.
pub struct SharedRef<T> {
    ptr: NonNull<SharedObject<T>>,
}

impl<T> SharedRef<T> {
    pub fn new(inner: &SharedObject<T>) -> SharedRef<T> {
        // FIXME: Who initializes SharedObject?

        // SAFETY: `inner` is a valid pointer.
        let ptr =
            unsafe { NonNull::new_unchecked(inner as *const _ as *mut _) };
        let sref = SharedRef { ptr };

        // SAFETY: The destructor of `sref` will decrement the reference
        //         counter.
        unsafe {
            sref.borrow_inner_mut().inc_ref();
        }

        sref
    }

    fn borrow_inner_mut(&self) -> GiantLockGuard<'_, RefCounted<NonNull<T>>> {
        (unsafe { self.ptr.as_ref() }).lock.borrow_mut()
    }

    /// Returns the physical address of the object value.
    pub fn paddr(this: &SharedRef<T>) -> PAddr {
        let nonnull = unsafe { &*this.borrow_inner_mut().as_mut() };

        let vaddr = VAddr::from_nonzero_usize(nonnull.addr());
        todo!()
    }

    /// Returns the mutable reference.
    ///
    /// **Warning:** This method may panic. See [`GiantLock::borrow_mut`]
    /// for more details.
    pub fn borrow_mut(&self) -> GiantLockGuard<'_, T> {
        GiantLockGuard::map(self.borrow_inner_mut(), |rc| {
            debug_assert!(rc.counter > 0);

            // SAFETY: GiantLockGuard ensures that only one thread can access
            //         the inner value at a time.
            unsafe { rc.object.as_mut() }
        })
    }

    /// Duplicates the reference.
    pub fn inc_ref(this: &SharedRef<T>) -> SharedRef<T> {
        // SAFETY: The destructor of `SharedRef` will decrement the reference
        //         counter.
        unsafe {
            this.borrow_inner_mut().inc_ref();
        }

        SharedRef { ptr: this.ptr }
    }

    pub unsafe fn dec_ref(this: &SharedRef<T>) {
        // SAFETY: The caller must ensure that it will not use the reference
        //         after calling this method.
        this.borrow_inner_mut().dec_ref();
    }

    pub unsafe fn leak(this: SharedRef<T>) {
        // SAFETY: The caller must ensure that it will
        mem::forget(this);
    }
}

impl<T> Drop for SharedRef<T> {
    fn drop(&mut self) {
        let mut rc = self.borrow_inner_mut();
        if rc.dec_ref() {
            // The reference counter reached zero. Free the memory.
            //
            // SAFETY: This reference was the last one, so we can safely
            //         free the memory.
            unsafe {
                let vaddr = VAddr::new(rc.object.as_ptr() as usize);

                // We should not keep the borrow guard to prevent use-after-free.
                drop(rc);

                memory_pool_mut(vaddr)
                    .unwrap()
                    .borrow_mut()
                    .free(vaddr)
                    .unwrap();
            }
        }
    }
}

/// A unique reference.
///
/// This is similar to [`Box<T>`]: it owns the inner value and drops it when
/// dropped.
pub struct UniqueRef<T> {
    object: NonNull<T>,
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

        Some(UniqueRef {
            object: sref.object,
        })
    }
}

impl<T> Deref for UniqueRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: The compiler guarantees the pointer is still alive and no
        //         mutable reference exists.
        unsafe { self.object.as_ref() }
    }
}

impl<T> DerefMut for UniqueRef<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: The compiler guarantees the pointer is still alive and no
        //         other references exist because this method requires `&mut self`.
        unsafe { self.object.as_mut() }
    }
}

impl<T> Drop for UniqueRef<T> {
    fn drop(&mut self) {
        // SAFETY: The check in `UniqueRef::new` ensures that this is the last
        //         reference.
        unsafe {
            let vaddr = VAddr::new(self.object.as_ptr() as usize);
            memory_pool_mut(vaddr)
                .unwrap()
                .borrow_mut()
                .free(vaddr)
                .unwrap();
        }
    }
}
