use core::{
    mem::{size_of, MaybeUninit, align_of},
    ops::{Deref, DerefMut},
    ptr::{drop_in_place, NonNull},
};

use essentials::{alignment::align_up, static_assert};

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
type SharedRefInner<T> = GiantLock<RefCounted<NonNull<T>>>;

// Make sure `SharedRefInner<T>`'s layout is the same regardless of `T`. This is
// crucial for making `SharedRefHeader` correct.
static_assert!(size_of::<SharedRefInner::<()>>() == size_of::<SharedRefInner::<[u8;512]>>());
static_assert!(align_of::<SharedRefInner::<()>>() == align_of::<SharedRefInner::<[u8;512]>>());

/// A opaque struct with `SharedRefInner<T>`'s layout (memory size and alignment).
///
/// This should be used with [`core::mem::MaybeUninit`] to mandate the user to
/// initialize the space.
#[repr(transparent)]
pub struct SharedRefHeader(GiantLock<RefCounted<NonNull<u8>>>);

/// A reference-counted mutably-borrowable reference.
///
/// This is similar to [`Arc<Mutex<T>>`]: allows multiple references to the
/// inner value by reference couting, and allows mutable access to the inner
/// value by big kernel lock + runtime borrow checking.
pub struct SharedRef<T> {
    ptr: NonNull<SharedRefInner<T>>,
}

impl<T> SharedRef<T> {
    /// Creates a new `SharedRef` pointing to the given memory space.
    pub unsafe fn new(
        vaddr: VAddr,
        num_pages: usize,
        value: T,
    ) -> SharedRef<T> {
        // Make sure MaybeUninit<T> doesn't have any memory overhead, so that
        // the casting below is safe.
        debug_assert!(
            size_of::<MaybeUninit<SharedRefInner<T>>>()
                == size_of::<SharedRefInner<T>>()
        );

        // We'll statically compute the # of pages to release in the
        // destructor. Make sure the # of pages we allocated is
        // correct.
        debug_assert!(num_pages == required_num_pages::<SharedRefInner<T>>());

        // Safety: The caller must ensure that the memory space is valid.
        let container =
            unsafe { vaddr.as_mut::<MaybeUninit<SharedRefInner<T>>>() };
        let inner = unsafe { NonNull::new_unchecked(vaddr.as_mut_ptr()) };
        container.write(GiantLock::new(RefCounted::new(inner)));

        // Safety: The container is initialized just above.
        let ptr = NonNull::from(unsafe { container.assume_init_ref() });

        // This check is very crucial as we'll turn `ptr` back into
        // `vaddr` when releasing the memory.
        debug_assert!(core::ptr::eq(ptr.as_ptr(), vaddr.as_ptr()));

        SharedRef { ptr }
    }

    fn borrow_inner_mut(&self) -> GiantLockGuard<'_, RefCounted<NonNull<T>>> {
        (unsafe { self.ptr.as_ref() }).borrow_mut()
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
    pub fn inc_ref(&self) -> SharedRef<T> {
        // Safety: The destructor of `SharedRef` will decrement the reference
        //         counter.
        unsafe {
            self.borrow_inner_mut().inc_ref();
        }

        SharedRef { ptr: self.ptr }
    }
}

impl<T> Drop for SharedRef<T> {
    fn drop(&mut self) {
        // Safety: The reference count was incremented when creating/cloning
        //         the reference.
        let needs_drop = unsafe { self.borrow_inner_mut().dec_ref() };

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
                let num_pages = required_num_pages::<SharedRefInner<T>>();
                retype_frames_as_unused(vaddr, num_pages);
            }
        }
    }
}

pub trait Destructor {
    type Target;

    fn drop(this: UniqueRef<Self::Target>);
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
    /// # Safety
    ///
    /// The caller must ensure that the pointer is unique.
    pub unsafe fn new(ptr: NonNull<T>) -> UniqueRef<T> {
        UniqueRef { ptr }
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
            // TODO:
        }
    }
}
