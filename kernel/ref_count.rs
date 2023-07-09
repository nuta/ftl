use core::{ops::Deref, ptr::NonNull, sync::atomic::AtomicUsize};

pub unsafe trait RefCounted {
    fn inc_ref(&self);
    fn dec_ref(&self);
}

pub struct OwnedRef<T: RefCounted + ?Sized> {
    ptr: NonNull<T>,
}

impl<T: RefCounted + ?Sized> OwnedRef<T> {
    pub const fn new(ptr: NonNull<T>) -> OwnedRef<T> {
        OwnedRef {
            ptr,
        }
    }

    pub fn inc_ref(&self) -> Self {
        // Safety: `self.ptr` is valid and the reference counter guarantees
        //         that the referenced value is still alive.
        unsafe { self.ptr.as_ref() }.inc_ref();
        Self { ptr: self.ptr }
    }
}

impl<T: RefCounted + ?Sized> Deref for OwnedRef<T> {
    type Target = T;

    fn deref(&self) -> &T {
        // Safety: `self.ptr` is valid and the reference counter guarantees
        //         that the referenced value is still alive.
        unsafe { self.ptr.as_ref() }
    }
}
