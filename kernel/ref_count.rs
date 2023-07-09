use core::{
    mem::{size_of, MaybeUninit},
    ptr::{drop_in_place, NonNull},
};

use essentials::alignment::align_up;

use crate::{
    address::VAddr,
    arch::PAGE_SIZE,
    giant_lock::{GiantLock, GiantLockGuard},
    memory_pool::retype_frames_as_unused,
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
        RefCounted { counter: 1, inner }
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
}

const fn required_num_pages<T>() -> usize {
    align_up(size_of::<T>(), PAGE_SIZE) / PAGE_SIZE
}

/// The type of the value a [`LockedRef`] points to.
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
/// deeply into a single useful object ([`LockedRef<T>`]).
type Container<T> = GiantLock<RefCounted<T>>;

/// A reference-counted mutably-borrowable reference. This is similar to
/// [`Arc<Mutex<T>>`] but it's optimized for our use case.
pub struct LockedRef<T> {
    ptr: NonNull<Container<T>>,
}

impl<T> LockedRef<T> {
    /// Creates a new `LockedRef` pointing to the given memory space.
    pub unsafe fn new(
        vaddr: VAddr,
        num_pages: usize,
        value: T,
    ) -> LockedRef<T> {
        // Make sure MaybeUninit<T> doesn't have any memory overhead, so that
        // the casting below is safe.
        debug_assert!(
            size_of::<MaybeUninit<Container<T>>>() == size_of::<Container<T>>()
        );

        // We'll statically compute the # of pages to release in the
        // destructor. Make sure the # of pages we allocated is
        // correct.
        debug_assert!(num_pages == required_num_pages::<Container<T>>());

        // Safety: The caller must ensure that the memory space is valid.
        let container = unsafe { vaddr.as_mut::<MaybeUninit<Container<T>>>() };
        container.write(GiantLock::new(RefCounted::new(value)));

        // Safety: The container is initialized just above.
        let ptr = NonNull::from(unsafe { container.assume_init_ref() });

        // This check is very crucial as we'll turn `ptr` back into
        // `vaddr` when releasing the memory.
        debug_assert!(core::ptr::eq(ptr.as_ptr(), vaddr.as_ptr()));

        LockedRef { ptr }
    }

    /// Returns the mutable reference.
    ///
    /// **Warning:** This method may panic. See [`GiantLock::borrow_mut`]
    /// for more details.
    pub fn borrow_mut(&self) -> GiantLockGuard<'_, RefCounted<T>> {
        (unsafe { self.ptr.as_ref() }).borrow_mut()
    }

    /// Duplicates the reference.
    pub fn inc_ref(&self) -> LockedRef<T> {
        // Safety: The destructor of `LockedRef` will decrement the reference
        //         counter.
        unsafe {
            self.borrow_mut().inc_ref();
        }

        LockedRef { ptr: self.ptr }
    }
}

impl<T> Drop for LockedRef<T> {
    fn drop(&mut self) {
        // Safety: The reference count was incremented when creating/cloning
        //         the reference.
        let needs_drop = unsafe { self.borrow_mut().dec_ref() };

        if needs_drop {
            // The reference counter reached zero. Drop the inner value
            // and free the memory.

            // Safety: This reference was the last one, so we can safely
            //         mutate the inner value and drop it.
            unsafe {
                // Call the destructor of the inner value (i.e. Drop).
                drop_in_place(self.ptr.as_mut());

                // Mark the memory frames as unused.
                let vaddr = VAddr::new(self.ptr.as_ptr() as usize);
                let num_pages = required_num_pages::<Container<T>>();
                retype_frames_as_unused(vaddr, num_pages);
            }
        }
    }
}
